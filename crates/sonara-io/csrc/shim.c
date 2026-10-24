/* Sonara's C shim around miniaudio's device layer (shim.h). It reproduces the prototype's
 * miniaudio usage in native/src/engine.cpp: the context, the enumeration and the device
 * configuration. */
#include "miniaudio_config.h"
#include "miniaudio.h"
#include "shim.h"

#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#if defined(__APPLE__)
#include <CoreAudio/CoreAudio.h>
#include <CoreFoundation/CoreFoundation.h>
#include <mach/mach.h>
#include <mach/mach_time.h>
#include <mach/thread_policy.h>
#include <os/workgroup.h>
#include <pthread.h> /* pthread_set_qos_class_self_np, through <pthread/qos.h> */
#endif

/* The shim's id and name arrays hold miniaudio's exactly, on every platform. */
_Static_assert(sizeof(ma_device_id) == SONARA_DEVICE_ID_BYTES, "ma_device_id is not 256 bytes");
_Static_assert(sizeof(((ma_device_info *)NULL)->name) == SONARA_DEVICE_NAME_BYTES,
               "ma_device_info.name is not 256 bytes");

struct sonara_context {
    ma_context context;
};

struct sonara_device {
    ma_device device;
    /* Copies of the requested ids: miniaudio reads them through pointers during init. */
    ma_device_id output_id;
    ma_device_id input_id;
    sonara_data_fn on_data;
    void *data_user;
    sonara_notification_fn on_notification;
    void *notification_user;
};

const char *sonara_miniaudio_version(void) { return ma_version_string(); }

const char *sonara_result_description(int32_t result) {
    return ma_result_description((ma_result)result);
}

uint32_t sonara_struct_size(uint32_t which) {
    switch (which) {
    case SONARA_STRUCT_DEVICE_INFO:
        return (uint32_t)sizeof(sonara_device_info);
    case SONARA_STRUCT_DEVICE_CONFIG:
        return (uint32_t)sizeof(sonara_device_config);
    case SONARA_STRUCT_DEVICE_FORMAT:
        return (uint32_t)sizeof(sonara_device_format);
    default:
        return 0;
    }
}

int32_t sonara_context_init(uint32_t kind, sonara_context **out) {
    sonara_context *self;
    ma_result result;

    *out = NULL;
    self = (sonara_context *)calloc(1, sizeof *self);
    if (self == NULL) {
        return MA_OUT_OF_MEMORY;
    }
    if (kind == SONARA_CONTEXT_NULL) {
        ma_backend backends[] = {ma_backend_null};
        result = ma_context_init(backends, 1, NULL, &self->context);
    } else {
        /* miniaudio's default list; it ends with the null backend. */
        result = ma_context_init(NULL, 0, NULL, &self->context);
    }
    if (result != MA_SUCCESS) {
        free(self);
        return result;
    }
    *out = self;
    return MA_SUCCESS;
}

void sonara_context_uninit(sonara_context *context) {
    if (context == NULL) {
        return;
    }
    ma_context_uninit(&context->context);
    free(context);
}

const char *sonara_context_backend_name(const sonara_context *context) {
    return ma_get_backend_name(context->context.backend);
}

/* The prototype's channels_of(), without its fallback of 2 (the Rust side applies it). */
static uint32_t native_channels(const ma_device_info *info) {
    uint32_t channels = 0;
    uint32_t count = info->nativeDataFormatCount;
    uint32_t i;
    const uint32_t capacity =
        (uint32_t)(sizeof info->nativeDataFormats / sizeof info->nativeDataFormats[0]);

    if (count > capacity) {
        count = capacity;
    }
    for (i = 0; i < count; ++i) {
        if (info->nativeDataFormats[i].channels > channels) {
            channels = info->nativeDataFormats[i].channels;
        }
    }
    return channels;
}

static void copy_info(sonara_device_info *dst, const ma_device_info *src, uint32_t kind) {
    memset(dst, 0, sizeof *dst);
    memcpy(dst->id, &src->id, sizeof dst->id);
    memcpy(dst->name, src->name, sizeof dst->name);
    dst->name[SONARA_DEVICE_NAME_BYTES - 1] = '\0';
    dst->kind = kind;
    dst->is_default = src->isDefault ? 1u : 0u;
    dst->channels = native_channels(src);
}

int32_t sonara_context_devices(sonara_context *context, sonara_device_info *out, uint32_t capacity,
                               uint32_t *count) {
    ma_device_info *playback = NULL;
    ma_device_info *capture = NULL;
    ma_uint32 playback_count = 0;
    ma_uint32 capture_count = 0;
    ma_uint32 i;
    ma_result result;

    *count = 0;
    result = ma_context_get_devices(&context->context, &playback, &playback_count, &capture,
                                    &capture_count);
    if (result != MA_SUCCESS) {
        return result;
    }
    /* The arrays stay valid until the next enumeration of this context; the Rust side
     * serializes every call on a context. */
    *count = playback_count + capture_count;
    for (i = 0; i < playback_count && i < capacity; ++i) {
        copy_info(&out[i], &playback[i], SONARA_DEVICE_PLAYBACK);
    }
    for (i = 0; i < capture_count && playback_count + i < capacity; ++i) {
        copy_info(&out[playback_count + i], &capture[i], SONARA_DEVICE_CAPTURE);
    }
    return MA_SUCCESS;
}

/* miniaudio's data callback: hands the buffers to the Rust trampoline in the shim's types. */
static void on_data(ma_device *device, void *output, const void *input, ma_uint32 frames) {
    const sonara_device *self = (const sonara_device *)device->pUserData;
    const uint32_t output_channels = output != NULL ? device->playback.channels : 0;
    const uint32_t input_channels = input != NULL ? device->capture.channels : 0;

    self->on_data(self->data_user, (float *)output, output_channels, (const float *)input,
                  input_channels, frames, device->sampleRate);
}

/* miniaudio's notification callback: forwards the notification as a SONARA_NOTIFICATION_*. */
static void on_notification(const ma_device_notification *notification) {
    const sonara_device *self = (const sonara_device *)notification->pDevice->pUserData;
    uint32_t kind;

    switch (notification->type) {
    case ma_device_notification_type_started:
        kind = SONARA_NOTIFICATION_STARTED;
        break;
    case ma_device_notification_type_stopped:
        kind = SONARA_NOTIFICATION_STOPPED;
        break;
    case ma_device_notification_type_rerouted:
        kind = SONARA_NOTIFICATION_REROUTED;
        break;
    case ma_device_notification_type_interruption_began:
        kind = SONARA_NOTIFICATION_INTERRUPTION_BEGAN;
        break;
    case ma_device_notification_type_interruption_ended:
        kind = SONARA_NOTIFICATION_INTERRUPTION_ENDED;
        break;
    case ma_device_notification_type_unlocked:
        kind = SONARA_NOTIFICATION_UNLOCKED;
        break;
    default:
        return;
    }
    self->on_notification(self->notification_user, kind);
}

int32_t sonara_device_init(sonara_context *context, const sonara_device_config *config,
                           sonara_device **out) {
    sonara_device *self;
    ma_device_config cfg;
    ma_result result;
    const int duplex = config->input_channels > 0;

    *out = NULL;
    if (config->on_data == NULL || config->on_notification == NULL) {
        return MA_INVALID_ARGS;
    }
    self = (sonara_device *)calloc(1, sizeof *self);
    if (self == NULL) {
        return MA_OUT_OF_MEMORY;
    }
    self->on_data = config->on_data;
    self->data_user = config->data_user;
    self->on_notification = config->on_notification;
    self->notification_user = config->notification_user;

    /* The prototype's configuration (native/src/engine.cpp, Engine::start). Fields it leaves at
     * ma_device_config_init()'s zero defaults are set to those same values here, to document
     * them. */
    cfg = ma_device_config_init(duplex ? ma_device_type_duplex : ma_device_type_playback);
    cfg.sampleRate = config->sample_rate;
    cfg.periodSizeInFrames = config->period_frames;
    cfg.periods = 0; /* default: MA_DEFAULT_PERIODS, 3 */
    cfg.performanceProfile = ma_performance_profile_low_latency;
    cfg.noPreSilencedOutputBuffer = MA_FALSE;
    cfg.noClip = MA_FALSE;             /* default: f32 output is clipped to [-1, 1] */
    cfg.noDisableDenormals = MA_FALSE; /* default: FTZ/DAZ around the callback on x86 (RT-11) */
    cfg.noFixedSizedCallback = MA_FALSE; /* default: every callback is exactly one period */
    cfg.playback.format = ma_format_f32;
    cfg.playback.channels = config->output_channels;
    cfg.playback.shareMode = ma_share_mode_shared;
    cfg.capture.format = ma_format_f32;
    cfg.capture.channels = duplex ? config->input_channels : 1;
    cfg.capture.shareMode = ma_share_mode_shared;
    cfg.dataCallback = on_data;
    cfg.notificationCallback = on_notification;
    cfg.pUserData = self;
    if (config->output_id != NULL) {
        memcpy(&self->output_id, config->output_id, sizeof self->output_id);
        cfg.playback.pDeviceID = &self->output_id;
    }
    if (duplex && config->input_id != NULL) {
        memcpy(&self->input_id, config->input_id, sizeof self->input_id);
        cfg.capture.pDeviceID = &self->input_id;
    }

    result = ma_device_init(&context->context, &cfg, &self->device);
    if (result != MA_SUCCESS) {
        free(self);
        return result;
    }
    *out = self;
    return MA_SUCCESS;
}

int32_t sonara_device_start(sonara_device *device) { return ma_device_start(&device->device); }

void sonara_device_get_format(const sonara_device *device, sonara_device_format *out) {
    const ma_device *d = &device->device;

    memset(out, 0, sizeof *out);
    out->sample_rate = d->sampleRate;
    out->period_frames = d->playback.internalPeriodSizeInFrames;
    out->periods = d->playback.internalPeriods;
    out->internal_sample_rate = d->playback.internalSampleRate;
    out->output_channels = d->playback.channels;
    if (d->type == ma_device_type_duplex) {
        out->capture_period_frames = d->capture.internalPeriodSizeInFrames;
        out->capture_periods = d->capture.internalPeriods;
    }
}

void sonara_device_uninit(sonara_device *device) {
    if (device == NULL) {
        return;
    }
    /* Stops the device first; returns once the callbacks have finished. */
    ma_device_uninit(&device->device);
    free(device);
}

int32_t sonara_thread_set_qos_user_interactive(void) {
#if defined(__APPLE__)
    /* A relative priority of 0: the class's own priority, the highest within it. */
    return (int32_t)pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0);
#else
    return MA_NOT_IMPLEMENTED;
#endif
}

#if defined(__APPLE__)
struct sonara_workgroup {
    /* Retained: the HAL hands the caller a reference, which sonara_thread_leave_audio_workgroup()
     * releases. */
    os_workgroup_t workgroup;
    os_workgroup_join_token_s token;
};

/* kAudioObjectPropertyElementMain (macOS 12; kAudioObjectPropertyElementMaster before), written
 * out so that the code needs no newer macOS than the workgroup API's 11. */
#define SONARA_ELEMENT_MAIN 0u

/* Reads a global property of an audio object into `data` of `size` bytes. */
static OSStatus get_property(AudioObjectID object, AudioObjectPropertySelector selector,
                             UInt32 qualifier_size, const void *qualifier, UInt32 size,
                             void *data) {
    const AudioObjectPropertyAddress address = {selector, kAudioObjectPropertyScopeGlobal,
                                                SONARA_ELEMENT_MAIN};
    return AudioObjectGetPropertyData(object, &address, qualifier_size, qualifier, &size, data);
}

/* The output device that `device_id` names: the default output device for NULL, else the device
 * whose UID the id holds (miniaudio's Core Audio device id is the UID, NUL-terminated). Returns 1
 * and stores the device in *out when there is one; else returns 0, with the HAL's status in
 * *detail when a call failed. */
static int find_output_device(const uint8_t *device_id, AudioObjectID *out, int32_t *detail) {
    AudioObjectID device = kAudioObjectUnknown;
    OSStatus status;

    if (device_id == NULL) {
        status = get_property(kAudioObjectSystemObject, kAudioHardwarePropertyDefaultOutputDevice,
                              0, NULL, sizeof device, &device);
    } else {
        char uid_text[SONARA_DEVICE_ID_BYTES + 1];
        CFStringRef uid;

        memcpy(uid_text, device_id, SONARA_DEVICE_ID_BYTES);
        uid_text[SONARA_DEVICE_ID_BYTES] = '\0';
        uid = CFStringCreateWithCString(kCFAllocatorDefault, uid_text, kCFStringEncodingUTF8);
        if (uid == NULL) {
            return 0; /* not UTF-8, so no device's UID */
        }
        status = get_property(kAudioObjectSystemObject, kAudioHardwarePropertyTranslateUIDToDevice,
                              sizeof uid, &uid, sizeof device, &device);
        CFRelease(uid);
    }
    if (status != kAudioHardwareNoError) {
        *detail = (int32_t)status;
        return 0;
    }
    /* An unknown UID translates successfully, to kAudioObjectUnknown. */
    if (device == kAudioObjectUnknown) {
        return 0;
    }
    *out = device;
    return 1;
}

/* The time-constraint policy for a period of `period_frames` at `sample_rate`, in Mach absolute
 * time: the period as period and constraint, half of it as computation (the ratio of Apple's
 * example in TN2169), preemptible. Returns 0 when the period does not fit the policy's fields. */
static int time_constraint_policy(uint32_t period_frames, Float64 sample_rate,
                                  thread_time_constraint_policy_data_t *out) {
    mach_timebase_info_data_t timebase;
    double ticks;

    if (mach_timebase_info(&timebase) != KERN_SUCCESS || timebase.numer == 0) {
        return 0;
    }
    ticks = (double)period_frames / sample_rate * 1e9 * (double)timebase.denom /
            (double)timebase.numer;
    if (!(ticks >= 2.0 && ticks <= (double)UINT32_MAX)) {
        return 0;
    }
    out->period = (uint32_t)ticks;
    out->computation = out->period / 2u;
    out->constraint = out->period;
    out->preemptible = TRUE;
    return 1;
}
#endif

uint32_t sonara_thread_join_audio_workgroup(const uint8_t *device_id, uint32_t period_frames,
                                            sonara_workgroup **out, int32_t *detail) {
    *out = NULL;
    *detail = 0;
#if defined(__APPLE__)
    if (__builtin_available(macOS 11.0, *)) {
        AudioObjectID device = kAudioObjectUnknown;
        Float64 sample_rate = 0.0;
        os_workgroup_t workgroup = NULL;
        thread_time_constraint_policy_data_t policy;
        thread_standard_policy_data_t standard = {0};
        const thread_act_t thread = pthread_mach_thread_np(pthread_self());
        sonara_workgroup *self;
        OSStatus status;
        kern_return_t kern;
        int joined;

        /* Everything that can fail without touching the thread comes first, so that a caller
         * can still fall back to a QoS class, which macOS refuses after a time-constraint
         * policy. */
        if (!find_output_device(device_id, &device, detail)) {
            return SONARA_WORKGROUP_NO_DEVICE;
        }
        status = get_property(device, kAudioDevicePropertyNominalSampleRate, 0, NULL,
                              sizeof sample_rate, &sample_rate);
        if (status != kAudioHardwareNoError) {
            *detail = (int32_t)status;
            return SONARA_WORKGROUP_NO_DEVICE;
        }
        if (!time_constraint_policy(period_frames, sample_rate, &policy)) {
            *detail = KERN_INVALID_ARGUMENT;
            return SONARA_WORKGROUP_POLICY_REFUSED;
        }
        status = get_property(device, kAudioDevicePropertyIOThreadOSWorkgroup, 0, NULL,
                              sizeof workgroup, &workgroup);
        if (status != kAudioHardwareNoError || workgroup == NULL) {
            *detail = (int32_t)status;
            return SONARA_WORKGROUP_NO_WORKGROUP;
        }
        if (os_workgroup_testcancel(workgroup)) {
            os_release(workgroup);
            return SONARA_WORKGROUP_CANCELLED;
        }
        self = (sonara_workgroup *)calloc(1, sizeof *self);
        if (self == NULL) {
            os_release(workgroup);
            return SONARA_WORKGROUP_OUT_OF_MEMORY;
        }

        /* The policy first: a thread joins a device's workgroup as a real-time thread. */
        kern = thread_policy_set(thread, THREAD_TIME_CONSTRAINT_POLICY, (thread_policy_t)&policy,
                                 THREAD_TIME_CONSTRAINT_POLICY_COUNT);
        if (kern != KERN_SUCCESS) {
            free(self);
            os_release(workgroup);
            *detail = (int32_t)kern;
            return SONARA_WORKGROUP_POLICY_REFUSED;
        }
        joined = os_workgroup_join(workgroup, &self->token);
        if (joined != 0) {
            /* EINVAL: cancelled since the test above; EALREADY: the thread is in a workgroup
             * this one does not nest with. The thread goes back to the standard policy. */
            (void)thread_policy_set(thread, THREAD_STANDARD_POLICY, (thread_policy_t)&standard,
                                    THREAD_STANDARD_POLICY_COUNT);
            free(self);
            os_release(workgroup);
            *detail = (int32_t)joined;
            return SONARA_WORKGROUP_JOIN_REFUSED;
        }
        self->workgroup = workgroup;
        *out = self;
        return SONARA_WORKGROUP_JOINED;
    }
    return SONARA_WORKGROUP_UNSUPPORTED;
#else
    (void)device_id;
    (void)period_frames;
    return SONARA_WORKGROUP_UNSUPPORTED;
#endif
}

void sonara_thread_leave_audio_workgroup(sonara_workgroup *membership) {
#if defined(__APPLE__)
    if (membership == NULL) {
        return;
    }
    /* A membership exists only where the join ran, on macOS 11 or later. */
    if (__builtin_available(macOS 11.0, *)) {
        os_workgroup_leave(membership->workgroup, &membership->token);
        os_release(membership->workgroup);
    }
    free(membership);
#else
    (void)membership;
#endif
}
