// What LibRaw reads but doesn't keep, kept as it reads the file (photo-library's record 0023): the
// fraction of a second of the time the photo was taken, EXIF's SubSecTimeOriginal (0x9291).
// LibRaw passes each tag of the EXIF directory to a callback, with its stream at the tag's value;
// its stream is a C++ object, so the callback is here, in C++, and Rust sees only plain C.
#include <cstring>

#include "libraw/libraw.h"

extern "C" {

struct rsraw_shot_details {
  /// SubSecTimeOriginal as written: digits, as "83", "943" or "669261"; empty where none.
  char subsec_original[16];
};

static void rsraw_exif_tag(void *context, int tag, int type, int len, unsigned int ord, void *ifp, INT64 base) {
  (void)type;
  (void)ord;
  (void)base;
  rsraw_shot_details *details = static_cast<rsraw_shot_details *>(context);
  // The EXIF directory's own tags come without a prefix (`parse_exif`); the first one read is the
  // photo's, as a RAF file's embedded JPEG's is its only one.
  if (tag != 0x9291 || len <= 0 || details->subsec_original[0] != 0)
    return;
  char text[sizeof details->subsec_original] = {0};
  const int count = len < int(sizeof text) - 1 ? len : int(sizeof text) - 1;
  static_cast<LibRaw_abstract_datastream *>(ifp)->read(text, 1, count);
  std::memcpy(details->subsec_original, text, sizeof text);
}

/// Called after `libraw_init` and before a file is opened: `details` is filled as LibRaw reads the
/// file, and must stay where it is until LibRaw's handle is closed.
void rsraw_watch_shot_details(libraw_data_t *data, rsraw_shot_details *details) {
  std::memset(details, 0, sizeof *details);
  libraw_set_exifparser_handler(data, rsraw_exif_tag, details);
}
}
