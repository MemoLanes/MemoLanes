#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
// EXTRA BEGIN
typedef struct DartCObject *WireSyncRust2DartDco;
typedef struct WireSyncRust2DartSse {
  uint8_t *ptr;
  int32_t len;
} WireSyncRust2DartSse;
// EXTRA END
typedef struct _Dart_Handle* Dart_Handle;

#define TILE_WIDTH_OFFSET 7

#define MAP_WIDTH (1 << MAP_WIDTH_OFFSET)

#define TILE_WIDTH (1 << TILE_WIDTH_OFFSET)

#define BITMAP_WIDTH_OFFSET 6

#define BITMAP_WIDTH (1 << BITMAP_WIDTH_OFFSET)

#define BITMAP_SIZE (uintptr_t)((BITMAP_WIDTH * BITMAP_WIDTH) / 8)

#define ZSTD_COMPRESS_LEVEL 3

#define DEFAULT_VIEW_SIZE_POWER 8

typedef struct wire_cst_list_prim_u_8_strict {
  uint8_t *ptr;
  int32_t len;
} wire_cst_list_prim_u_8_strict;

typedef struct wire_cst_render_result {
  double left;
  double top;
  double right;
  double bottom;
  struct wire_cst_list_prim_u_8_strict *data;
} wire_cst_render_result;

typedef struct wire_cst_raw_data_file {
  struct wire_cst_list_prim_u_8_strict *name;
  struct wire_cst_list_prim_u_8_strict *path;
} wire_cst_raw_data_file;

typedef struct wire_cst_list_raw_data_file {
  struct wire_cst_raw_data_file *ptr;
  int32_t len;
} wire_cst_list_raw_data_file;

void frbgen_project_dv_dart_fn_deliver_output(int32_t call_id,
                                              uint8_t *ptr_,
                                              int32_t rust_vec_len_,
                                              int32_t data_len_);

void frbgen_project_dv_wire_finalize_ongoing_journey(int64_t port_);

void frbgen_project_dv_wire_get_raw_data_mode(int64_t port_);

void frbgen_project_dv_wire_init(int64_t port_,
                                 struct wire_cst_list_prim_u_8_strict *temp_dir,
                                 struct wire_cst_list_prim_u_8_strict *doc_dir,
                                 struct wire_cst_list_prim_u_8_strict *support_dir,
                                 struct wire_cst_list_prim_u_8_strict *cache_dir);

void frbgen_project_dv_wire_list_all_raw_data(int64_t port_);

void frbgen_project_dv_wire_on_location_update(int64_t port_,
                                               double latitude,
                                               double longitude,
                                               int64_t timestamp_ms,
                                               float accuracy,
                                               float *altitude,
                                               float *speed);

void frbgen_project_dv_wire_render_map_overlay(int64_t port_,
                                               float zoom,
                                               double left,
                                               double top,
                                               double right,
                                               double bottom);

void frbgen_project_dv_wire_toggle_raw_data_mode(int64_t port_, bool enable);

float *frbgen_project_dv_cst_new_box_autoadd_f_32(float value);

struct wire_cst_render_result *frbgen_project_dv_cst_new_box_autoadd_render_result(void);

struct wire_cst_list_prim_u_8_strict *frbgen_project_dv_cst_new_list_prim_u_8_strict(int32_t len);

struct wire_cst_list_raw_data_file *frbgen_project_dv_cst_new_list_raw_data_file(int32_t len);
static int64_t dummy_method_to_enforce_bundling(void) {
    int64_t dummy_var = 0;
    dummy_var ^= ((int64_t) (void*) drop_dart_object);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_cst_new_box_autoadd_f_32);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_cst_new_box_autoadd_render_result);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_cst_new_list_prim_u_8_strict);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_cst_new_list_raw_data_file);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_dart_fn_deliver_output);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_wire_finalize_ongoing_journey);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_wire_get_raw_data_mode);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_wire_init);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_wire_list_all_raw_data);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_wire_on_location_update);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_wire_render_map_overlay);
    dummy_var ^= ((int64_t) (void*) frbgen_project_dv_wire_toggle_raw_data_mode);
    dummy_var ^= ((int64_t) (void*) get_dart_object);
    dummy_var ^= ((int64_t) (void*) new_dart_opaque);
    dummy_var ^= ((int64_t) (void*) store_dart_post_cobject);
    return dummy_var;
}
