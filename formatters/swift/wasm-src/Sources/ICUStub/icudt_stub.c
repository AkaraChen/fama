/*
 * Minimal ICU common-data ("CmnD") stub replacing the ~32.4 MiB icudt76l
 * archive statically linked by FoundationICU.
 *
 * Swift's standard library carries its own Unicode tables; swift-format only
 * parses/prints Swift tokens and does not need ICU locale, timezone, currency,
 * collation, or legacy charset converter tables. Because this C target is
 * linked before the SwiftWasm SDK's static archives, wasm-ld resolves the
 * icudt*_dat symbol here and never pulls the 34 MiB data object from the SDK.
 *
 * build-ci.sh inspects the installed SwiftWasm SDK archives via llvm-nm and
 * regenerates this file if the SDK uses a different versioned symbol name
 * (e.g. icudt_swift76_dat).
 */
#include <stdint.h>

typedef struct {
  uint16_t headerSize;
  uint8_t magic1;
  uint8_t magic2;
  struct {
    uint16_t size;
    uint16_t reservedWord;
    uint8_t isBigEndian;
    uint8_t charsetFamily;
    uint8_t sizeofUChar;
    uint8_t reservedByte;
    uint8_t dataFormat[4];
    uint8_t formatVersion[4];
    uint8_t dataVersion[4];
  } info;
  uint8_t padding[8];
  uint32_t count;
  uint32_t reserved;
  const void *fakeNameAndData[8];
} ICUStubDataHeader;

#define DEFINE_ICUDT_STUB(sym)                                                 \
  __attribute__((used, aligned(16), visibility("default")))                    \
  const ICUStubDataHeader sym = {                                              \
      32,                                                                      \
      0xda,                                                                    \
      0x27,                                                                    \
      {                                                                        \
          20,                                                                  \
          0,                                                                   \
          0,                                                                   \
          0,                                                                   \
          2,                                                                   \
          0,                                                                   \
          {'C', 'm', 'n', 'D'},                                                \
          {1, 0, 0, 0},                                                        \
          {3, 0, 0, 0},                                                        \
      },                                                                       \
      {0},                                                                     \
      0,                                                                       \
      0,                                                                       \
      {0},                                                                     \
  }

DEFINE_ICUDT_STUB(icudt76_dat);
