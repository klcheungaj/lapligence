// llg_container.h -- scheduler-independent unpacked container storage.
#ifndef LLG_CONTAINER_H
#define LLG_CONTAINER_H

#include "llg_value.h"
#include "llg_string.h"

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum {
    LLG_CONTAINER_REDUCE_SUM = 0,
    LLG_CONTAINER_REDUCE_PRODUCT = 1,
    LLG_CONTAINER_REDUCE_AND = 2,
    LLG_CONTAINER_REDUCE_OR = 3,
    LLG_CONTAINER_REDUCE_XOR = 4,
};

typedef void (*llg_container_notify_fn)(sv4_t* contents, sv4_t* shape,
                                        int change);

enum {
    LLG_VALUE_PACKED = 0,
    LLG_VALUE_REAL = 1,
    LLG_VALUE_STRING = 2,
    LLG_VALUE_CHANDLE = 3,
    LLG_VALUE_EVENT = 4,
    LLG_VALUE_AGGREGATE = 5,
    LLG_VALUE_FIXED_ARRAY = 6,
    LLG_VALUE_CONTAINER = 7,
    LLG_VALUE_OPAQUE = 8,
};

typedef struct llg_value_desc_t llg_value_desc_t;
typedef struct llg_value_member_desc_t llg_value_member_desc_t;
typedef struct llg_dyn_value_array_t llg_dyn_value_array_t;
typedef struct llg_value_t llg_value_t;

struct llg_value_member_desc_t {
    const llg_value_desc_t* value;
};

struct llg_value_desc_t {
    uint8_t kind;
    uint64_t type_id;
    uint32_t packed_width;
    int8_t packed_signed;
    uint8_t packed_two_state;
    size_t item_count;
    const llg_value_desc_t* element;
    const llg_value_member_desc_t* members;
    size_t member_count;
};

struct llg_value_t {
    const llg_value_desc_t* desc;
    union {
        sv4_t packed;
        double real;
        llg_string_t string;
        void* handle;
        llg_dyn_value_array_t* container;
        llg_value_t* items;
    } value;
};

struct llg_dyn_value_array_t {
    llg_value_t* data;
    size_t size;
    const llg_value_desc_t* element;
    sv4_t* contents_dependency;
    sv4_t* shape_dependency;
    llg_container_notify_fn notify;
};

void llg_dyn_value_init(llg_dyn_value_array_t* array,
                        const llg_value_desc_t* element);
void llg_dyn_value_destroy(llg_dyn_value_array_t* array);
void llg_dyn_value_delete(llg_dyn_value_array_t* array);
void llg_dyn_value_copy(llg_dyn_value_array_t* dst,
                        const llg_dyn_value_array_t* src);
void llg_dyn_value_new(llg_dyn_value_array_t* dst, sv4_t size,
                       const llg_dyn_value_array_t* initializer);
void llg_dyn_value_assign_reals(llg_dyn_value_array_t* dst,
                                const double* values, size_t count);
void llg_dyn_value_assign_strings(llg_dyn_value_array_t* dst,
                                  llg_string_t* values, size_t count);
void llg_dyn_value_assign_chandles(llg_dyn_value_array_t* dst,
                                   void* const* values, size_t count);
size_t llg_dyn_value_size(const llg_dyn_value_array_t* array);
double llg_dyn_value_get_real(const llg_dyn_value_array_t* array, sv4_t index);
llg_string_t llg_dyn_value_get_string(const llg_dyn_value_array_t* array,
                                      sv4_t index);
void* llg_dyn_value_get_chandle(const llg_dyn_value_array_t* array,
                                sv4_t index);
sv4_t llg_dyn_value_get_nested(const llg_dyn_value_array_t* array,
                               const sv4_t* indices, size_t count);
double llg_dyn_value_get_nested_real(const llg_dyn_value_array_t* array,
                                     const sv4_t* indices, size_t count);
llg_string_t llg_dyn_value_get_nested_string(
    const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count);
void* llg_dyn_value_get_nested_chandle(
    const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count);
int llg_dyn_value_set_real(llg_dyn_value_array_t* array, sv4_t index,
                           double value);
int llg_dyn_value_set_string(llg_dyn_value_array_t* array, sv4_t index,
                             llg_string_t value);
int llg_dyn_value_set_chandle(llg_dyn_value_array_t* array, sv4_t index,
                              void* value);
int llg_dyn_value_set_nested(llg_dyn_value_array_t* array,
                             const sv4_t* indices, size_t count, sv4_t value);
int llg_dyn_value_set_nested_real(llg_dyn_value_array_t* array,
                                  const sv4_t* indices, size_t count,
                                  double value);
int llg_dyn_value_set_nested_string(llg_dyn_value_array_t* array,
                                    const sv4_t* indices, size_t count,
                                    llg_string_t value);
int llg_dyn_value_set_nested_chandle(llg_dyn_value_array_t* array,
                                     const sv4_t* indices, size_t count,
                                     void* value);
int llg_dyn_value_set_nested_container(
    llg_dyn_value_array_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_value_array_t* source);

enum {
    LLG_CONTAINER_CHANGED_CONTENTS = 1,
    LLG_CONTAINER_CHANGED_SHAPE = 2,
};

typedef struct {
    sv4_t* data;
    size_t size;
    uint32_t element_width;
    int8_t element_signed;
    uint8_t element_two_state;
    sv4_t* contents_dependency;
    sv4_t* shape_dependency;
    llg_container_notify_fn notify;
} llg_dyn_array_t;

int llg_dyn_value_set_nested_container_from_packed(
    llg_dyn_value_array_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_array_t* source);

void llg_dyn_init(llg_dyn_array_t* array, uint32_t element_width,
                  int8_t element_signed, int element_two_state);
void llg_dyn_destroy(llg_dyn_array_t* array);
void llg_dyn_delete(llg_dyn_array_t* array);
void llg_dyn_copy(llg_dyn_array_t* dst, const llg_dyn_array_t* src);
void llg_dyn_assign_values(llg_dyn_array_t* dst, const sv4_t* values,
                           size_t count);
void llg_dyn_new(llg_dyn_array_t* dst, sv4_t size,
                 const llg_dyn_array_t* initializer);
void llg_dyn_resize(llg_dyn_array_t* array, sv4_t size);
size_t llg_dyn_size(const llg_dyn_array_t* array);
sv4_t llg_dyn_get(const llg_dyn_array_t* array, sv4_t index);
int llg_dyn_set(llg_dyn_array_t* array, sv4_t index, sv4_t value);
sv4_t llg_dyn_reduce(const llg_dyn_array_t* array, int operation);

typedef struct {
    sv4_t* data;
    size_t size;
    size_t capacity;
    // Maximum element count; SIZE_MAX denotes an unbounded queue.
    size_t limit;
    uint32_t element_width;
    int8_t element_signed;
    uint8_t element_two_state;
    sv4_t* contents_dependency;
    sv4_t* shape_dependency;
    llg_container_notify_fn notify;
} llg_queue_t;

void llg_queue_init(llg_queue_t* queue, uint32_t element_width,
                    int8_t element_signed, int element_two_state,
                    uint64_t maximum_elements);
void llg_queue_destroy(llg_queue_t* queue);
void llg_queue_delete(llg_queue_t* queue);
void llg_queue_copy(llg_queue_t* dst, const llg_queue_t* src);
void llg_queue_assign_values(llg_queue_t* dst, const sv4_t* values,
                             size_t count);
size_t llg_queue_size(const llg_queue_t* queue);
sv4_t llg_queue_get(const llg_queue_t* queue, sv4_t index);
int llg_queue_set(llg_queue_t* queue, sv4_t index, sv4_t value);
int llg_queue_insert(llg_queue_t* queue, sv4_t index, sv4_t value);
int llg_queue_delete_index(llg_queue_t* queue, sv4_t index);
void llg_queue_push_front(llg_queue_t* queue, sv4_t value);
void llg_queue_push_back(llg_queue_t* queue, sv4_t value);
sv4_t llg_queue_pop_front(llg_queue_t* queue);
sv4_t llg_queue_pop_back(llg_queue_t* queue);
sv4_t llg_queue_front(const llg_queue_t* queue);
sv4_t llg_queue_back(const llg_queue_t* queue);
sv4_t llg_queue_reduce(const llg_queue_t* queue, int operation);

enum {
    LLG_ASSOC_INTEGRAL = 0,
    LLG_ASSOC_STRING = 1,
};

typedef struct {
    sv4_t integral_key;
    unsigned char* string_key;
    size_t string_length;
    sv4_t value;
} llg_assoc_entry_t;

typedef struct {
    llg_assoc_entry_t* entries;
    size_t size;
    size_t capacity;
    uint32_t element_width;
    int8_t element_signed;
    uint8_t element_two_state;
    uint8_t key_kind;
    // Zero denotes the wildcard integral index type. Wildcard keys are
    // canonicalized to the model width; traversal is rejected because IEEE
    // 1800-2009 7.8.1 forbids it.
    uint32_t key_width;
    int8_t key_signed;
    uint8_t key_two_state;
    // Missing associative entries read this value without allocating an
    // entry.  `has_default_value` distinguishes an explicit assignment
    // pattern default from the element-type default for copy semantics.
    sv4_t default_value;
    uint8_t has_default_value;
    sv4_t* contents_dependency;
    sv4_t* shape_dependency;
    llg_container_notify_fn notify;
} llg_assoc_t;

void llg_assoc_init_integral(llg_assoc_t* array, uint32_t element_width,
                             int8_t element_signed, int element_two_state,
                             uint32_t key_width, int8_t key_signed,
                             int key_two_state);
void llg_assoc_init_string(llg_assoc_t* array, uint32_t element_width,
                           int8_t element_signed, int element_two_state);
void llg_assoc_destroy(llg_assoc_t* array);
void llg_assoc_delete(llg_assoc_t* array);
void llg_assoc_copy(llg_assoc_t* dst, const llg_assoc_t* src);
size_t llg_assoc_count(const llg_assoc_t* array);
sv4_t llg_assoc_reduce(const llg_assoc_t* array, int operation);

sv4_t llg_assoc_get_integral(const llg_assoc_t* array, sv4_t key);
int llg_assoc_set_integral(llg_assoc_t* array, sv4_t key, sv4_t value);
int llg_assoc_exists_integral(const llg_assoc_t* array, sv4_t key);
int llg_assoc_delete_integral(llg_assoc_t* array, sv4_t key);
void llg_assoc_set_default(llg_assoc_t* array, sv4_t value);
void llg_assoc_reset_default(llg_assoc_t* array);
int llg_assoc_first_integral(const llg_assoc_t* array, sv4_t* key);
int llg_assoc_last_integral(const llg_assoc_t* array, sv4_t* key);
int llg_assoc_next_integral(const llg_assoc_t* array, sv4_t* key);
int llg_assoc_prev_integral(const llg_assoc_t* array, sv4_t* key);

sv4_t llg_assoc_get_string(const llg_assoc_t* array, const void* key,
                           size_t key_length);
int llg_assoc_set_string(llg_assoc_t* array, const void* key, size_t key_length,
                         sv4_t value);
int llg_assoc_exists_string(const llg_assoc_t* array, const void* key,
                            size_t key_length);
int llg_assoc_delete_string(llg_assoc_t* array, const void* key,
                            size_t key_length);
int llg_assoc_first_string(const llg_assoc_t* array, const unsigned char** key,
                           size_t* key_length);
int llg_assoc_last_string(const llg_assoc_t* array, const unsigned char** key,
                          size_t* key_length);
int llg_assoc_next_string(const llg_assoc_t* array, const void* current,
                          size_t current_length, const unsigned char** key,
                          size_t* key_length);
int llg_assoc_prev_string(const llg_assoc_t* array, const void* current,
                          size_t current_length, const unsigned char** key,
                          size_t* key_length);

#ifdef __cplusplus
}
#endif

#endif // LLG_CONTAINER_H
