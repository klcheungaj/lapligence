// llg_container.h -- scheduler-independent unpacked container storage.
#ifndef LLG_CONTAINER_H
#define LLG_CONTAINER_H

#include "llg_value.h"

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

typedef struct {
    sv4_t* data;
    size_t size;
    uint32_t element_width;
    int8_t element_signed;
    uint8_t element_two_state;
} llg_dyn_array_t;

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
    // Zero denotes the wildcard integral index type. Wildcard traversal is
    // deliberately rejected because IEEE 1800-2009 7.8.1 forbids it.
    uint32_t key_width;
    int8_t key_signed;
    uint8_t key_two_state;
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
