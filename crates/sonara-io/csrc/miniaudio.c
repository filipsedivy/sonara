/* Single translation unit that compiles the vendored miniaudio implementation
 * (third_party/miniaudio) with Sonara's switches (miniaudio_config.h). */
#include "miniaudio_config.h"
#define MINIAUDIO_IMPLEMENTATION
#include "miniaudio.h"
