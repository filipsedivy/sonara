/* The miniaudio switches Sonara compiles with: the same as the prototype's build
 * (native/src/miniaudio_impl.c). Sonara uses miniaudio only for device I/O; decoding and encoding
 * live in sonara-media.
 *
 * Every translation unit that includes miniaudio.h includes this file first, so that the
 * implementation (miniaudio.c) and the shim (shim.c) see the same declarations and struct
 * layouts. */
#ifndef SONARA_MINIAUDIO_CONFIG_H
#define SONARA_MINIAUDIO_CONFIG_H

#define MA_NO_DECODING
#define MA_NO_ENCODING
#define MA_NO_RESOURCE_MANAGER
#define MA_NO_NODE_GRAPH
#define MA_NO_ENGINE
#define MA_NO_GENERATION

#endif
