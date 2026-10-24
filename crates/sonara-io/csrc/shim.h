/* Sonara's narrow C shim around miniaudio's device layer.
 *
 * miniaudio's context and device structs differ per platform, so Rust never sees them: the shim
 * owns them behind two opaque handles, and every type in this header is made of fixed-width
 * integers, fixed-size byte arrays, `float` and pointers. The Rust declarations of this header
 * (src/ffi/shim.rs) are written by hand and must change together with it (RT-10).
 *
 * Every device function that can fail returns a miniaudio `ma_result` code (0 is success);
 * sonara_result_description() turns one into miniaudio's text. The thread functions report the
 * system's own codes instead: sonara_thread_set_qos_user_interactive() an error number, and
 * sonara_thread_join_audio_workgroup() one of SONARA_WORKGROUP_* with the system's code as its
 * detail. */
#ifndef SONARA_SHIM_H
#define SONARA_SHIM_H

#include <stdint.h>

/* Bytes of a device id: the raw bytes of miniaudio's `ma_device_id` union. */
#define SONARA_DEVICE_ID_BYTES 256
/* Bytes of a device name, NUL terminator included. */
#define SONARA_DEVICE_NAME_BYTES 256

/* Context kinds (sonara_context_init). */
#define SONARA_CONTEXT_DEFAULT 0u /* miniaudio's default backend list */
#define SONARA_CONTEXT_NULL 1u    /* the null backend only: a simulated real-time device */

/* Device kinds (sonara_device_info.kind). */
#define SONARA_DEVICE_PLAYBACK 0u
#define SONARA_DEVICE_CAPTURE 1u

/* Device notifications (sonara_notification_fn). */
#define SONARA_NOTIFICATION_STARTED 0u
#define SONARA_NOTIFICATION_STOPPED 1u
#define SONARA_NOTIFICATION_REROUTED 2u
#define SONARA_NOTIFICATION_INTERRUPTION_BEGAN 3u
#define SONARA_NOTIFICATION_INTERRUPTION_ENDED 4u
#define SONARA_NOTIFICATION_UNLOCKED 5u

/* Struct ids for sonara_struct_size(). */
#define SONARA_STRUCT_DEVICE_INFO 0u
#define SONARA_STRUCT_DEVICE_CONFIG 1u
#define SONARA_STRUCT_DEVICE_FORMAT 2u

typedef struct sonara_context sonara_context;
typedef struct sonara_device sonara_device;

/* One device of an enumeration. */
typedef struct sonara_device_info {
    /* The raw bytes of the `ma_device_id`. */
    uint8_t id[SONARA_DEVICE_ID_BYTES];
    /* The device name, NUL-terminated. */
    char name[SONARA_DEVICE_NAME_BYTES];
    /* SONARA_DEVICE_PLAYBACK or SONARA_DEVICE_CAPTURE. */
    uint32_t kind;
    /* 1 for the system default device of its kind, else 0. */
    uint32_t is_default;
    /* The most channels among the device's native formats; 0 when it lists none. */
    uint32_t channels;
} sonara_device_info;

/* The device thread's data callback: `frames` frames of interleaved `float` output with
 * `output_channels` channels to fill, and input with `input_channels` channels, or NULL and 0
 * without input. `sample_rate` is the device's sample rate. */
typedef void (*sonara_data_fn)(void *user, float *output, uint32_t output_channels,
                               const float *input, uint32_t input_channels, uint32_t frames,
                               uint32_t sample_rate);

/* A device notification: one of SONARA_NOTIFICATION_*. */
typedef void (*sonara_notification_fn)(void *user, uint32_t notification);

/* What sonara_device_init() opens. */
typedef struct sonara_device_config {
    uint32_t sample_rate;
    /* The period in frames; every data callback gets exactly this many frames. */
    uint32_t period_frames;
    uint32_t output_channels;
    /* 0 opens a playback device; more opens a duplex device. */
    uint32_t input_channels;
    /* NULL: the default device; else SONARA_DEVICE_ID_BYTES bytes of a device id. */
    const uint8_t *output_id;
    /* As output_id; ignored without input channels. */
    const uint8_t *input_id;
    sonara_data_fn on_data;
    void *data_user;
    sonara_notification_fn on_notification;
    void *notification_user;
} sonara_device_config;

/* What an initialized device obtained. */
typedef struct sonara_device_format {
    /* The device's sample rate: the requested one (miniaudio converts). */
    uint32_t sample_rate;
    /* The playback period in frames, and the number of periods. */
    uint32_t period_frames;
    uint32_t periods;
    /* The playback hardware's sample rate. */
    uint32_t internal_sample_rate;
    /* The playback channels of the data callback. */
    uint32_t output_channels;
    /* The capture period in frames and number of periods; 0 for a playback device. */
    uint32_t capture_period_frames;
    uint32_t capture_periods;
} sonara_device_format;

/* miniaudio's version string, for example "0.11.25". */
const char *sonara_miniaudio_version(void);

/* miniaudio's description of a result code. Never NULL. */
const char *sonara_result_description(int32_t result);

/* sizeof of one of the structs above (SONARA_STRUCT_*), 0 for an unknown id. The Rust side checks
 * its hand-written declarations against it. */
uint32_t sonara_struct_size(uint32_t which);

/* Creates a context of `kind` (SONARA_CONTEXT_*) in *out. */
int32_t sonara_context_init(uint32_t kind, sonara_context **out);

/* Uninitializes and frees a context. Its device must be uninitialized first. */
void sonara_context_uninit(sonara_context *context);

/* miniaudio's name of the context's backend, for example "Core Audio" or "Null". */
const char *sonara_context_backend_name(const sonara_context *context);

/* Enumerates the devices: playback devices first, then capture devices. Writes up to `capacity`
 * of them to `out` and their total to *count; call again with more room when *count exceeds
 * `capacity`. */
int32_t sonara_context_devices(sonara_context *context, sonara_device_info *out, uint32_t capacity,
                               uint32_t *count);

/* Initializes a device with the prototype's miniaudio configuration (shim.c) in *out. The
 * device's callbacks may run from sonara_device_start() until sonara_device_uninit() returns. */
int32_t sonara_device_init(sonara_context *context, const sonara_device_config *config,
                           sonara_device **out);

/* Starts the device. */
int32_t sonara_device_start(sonara_device *device);

/* The format an initialized device obtained. */
void sonara_device_get_format(const sonara_device *device, sonara_device_format *out);

/* Stops, uninitializes and frees a device. Blocks until its callback has returned; never call it
 * from a callback. */
void sonara_device_uninit(sonara_device *device);

/* Requests QOS_CLASS_USER_INTERACTIVE for the calling thread: the class of work the user is
 * waiting for. The player's render thread calls it once, before its loop; it is a
 * system call, so never call it from real-time code (RT-03).
 *
 * macOS honours the class only in a task with an application role, such as an app launched by
 * LaunchServices. A process without a role, such as a tool started from a terminal, gets the
 * class squashed to the default class (base priority 31 instead of 46 or 47), and the call still
 * succeeds.
 *
 * Returns 0 when the class was requested. On macOS a failure returns the error number of
 * pthread_set_qos_class_self_np(), for example EPERM for a thread with a fixed scheduling policy.
 * Elsewhere it does nothing and returns MA_NOT_IMPLEMENTED (-29): only macOS has QoS classes. */
int32_t sonara_thread_set_qos_user_interactive(void);

/* Results of sonara_thread_join_audio_workgroup(), with what its `detail` holds. */
#define SONARA_WORKGROUP_JOINED 0u         /* joined; detail 0 */
#define SONARA_WORKGROUP_UNSUPPORTED 1u    /* not macOS 11 or later; detail 0 */
#define SONARA_WORKGROUP_NO_DEVICE 2u      /* no such Core Audio device; the HAL's OSStatus, or 0 */
#define SONARA_WORKGROUP_NO_WORKGROUP 3u   /* the device has no IO workgroup; the HAL's OSStatus */
#define SONARA_WORKGROUP_CANCELLED 4u      /* the workgroup was cancelled; detail 0 */
#define SONARA_WORKGROUP_POLICY_REFUSED 5u /* thread_policy_set refused; its kern_return_t */
#define SONARA_WORKGROUP_JOIN_REFUSED 6u   /* os_workgroup_join refused; its error number */
#define SONARA_WORKGROUP_OUT_OF_MEMORY 7u  /* detail 0 */

/* The calling thread's membership of an audio device's IO workgroup. */
typedef struct sonara_workgroup sonara_workgroup;

/* Joins the calling thread to the IO thread workgroup of a Core Audio output device
 * (kAudioDevicePropertyIOThreadOSWorkgroup), so that the system schedules it as part of the
 * device's audio deadline. `device_id` is NULL for the system's default output
 * device, else SONARA_DEVICE_ID_BYTES bytes of a device id: on Core Audio, the device's UID.
 *
 * In this order: it resolves the device, reads its nominal sample rate and its workgroup, gives
 * the thread a time-constraint policy (THREAD_TIME_CONSTRAINT_POLICY: a period of
 * `period_frames` at that rate, half of it as computation, the whole period as constraint,
 * preemptible), and joins the workgroup with os_workgroup_join(). The policy comes first, as
 * Apple's developer support advises for a thread that joins a device's workgroup.
 *
 * The workgroup belongs to the device, not to an open stream: it is the same object before a
 * stream opens, while it runs and after it closes, so a thread may join it before miniaudio opens
 * the device.
 *
 * Returns SONARA_WORKGROUP_JOINED and stores the membership in *out, which the same thread
 * passes to sonara_thread_leave_audio_workgroup() before it exits. Otherwise *out is NULL, and
 * `detail` holds the system's code (see SONARA_WORKGROUP_*). Every failure before the join
 * leaves the thread as it was. A refused join puts the thread back on the standard policy, but
 * macOS then refuses it a QoS class (EPERM), as it does for every thread that has had a
 * time-constraint policy.
 *
 * It makes system calls and allocates, so call it once on a thread before its real-time loop,
 * never from real-time code (RT-03). Elsewhere it does nothing and returns
 * SONARA_WORKGROUP_UNSUPPORTED: only macOS has audio workgroups. */
uint32_t sonara_thread_join_audio_workgroup(const uint8_t *device_id, uint32_t period_frames,
                                            sonara_workgroup **out, int32_t *detail);

/* Leaves the workgroup that sonara_thread_join_audio_workgroup() joined, releases it and frees
 * the membership. Call it on the thread that joined, before the thread exits; a thread leaves
 * its workgroups in the reverse order of joining them. NULL does nothing. The time-constraint
 * policy stays until the thread exits. */
void sonara_thread_leave_audio_workgroup(sonara_workgroup *membership);

#endif
