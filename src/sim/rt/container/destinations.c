/* Destination-passing `_to` forms; private fragment, see value/destinations.h. */
void llg_dyn_value_get_nested_to(sv4_t* dst, const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count) {
    sv4_replace(dst, llg_dyn_value_get_nested(array, indices, count));
}
void llg_fixed_stream_source_to(sv4_t* dst, const sv4_t* values, int64_t declaration_left, int64_t declaration_right, uint32_t element_width, int element_two_state, int selector_kind, const sv4_t* first, const sv4_t* second) {
    sv4_replace(dst, llg_fixed_stream_source(values, declaration_left, declaration_right, element_width, element_two_state, selector_kind, *first, *second));
}
void llg_stream_to_fixed_to(sv4_t* dst, const sv4_t* value, uint32_t width, int is_signed) {
    sv4_replace(dst, llg_stream_to_fixed(*value, width, is_signed));
}
void llg_queue_value_get_to(sv4_t* dst, const llg_queue_value_array_t* queue, const sv4_t* index) {
    sv4_replace(dst, llg_queue_value_get(queue, *index));
}
void llg_queue_value_get_nested_to(sv4_t* dst, const llg_queue_value_array_t* queue, const sv4_t* indices, size_t count) {
    sv4_replace(dst, llg_queue_value_get_nested(queue, indices, count));
}
void llg_dyn_stream_to(sv4_t* dst, const llg_dyn_array_t* array, uint32_t slice, int right_to_left, int selector_kind, const sv4_t* first, const sv4_t* second) {
    sv4_replace(dst, llg_dyn_stream(array, slice, right_to_left, selector_kind, *first, *second));
}
void llg_dyn_get_to(sv4_t* dst, const llg_dyn_array_t* array, const sv4_t* index) {
    sv4_replace(dst, llg_dyn_get(array, *index));
}
void llg_dyn_reduce_to(sv4_t* dst, const llg_dyn_array_t* array, int operation) {
    sv4_replace(dst, llg_dyn_reduce(array, operation));
}
void llg_dyn_reduce_with_to(sv4_t* dst, const llg_dyn_array_t* array, int operation, uint32_t result_width, int8_t result_signed, int result_two_state, llg_container_eval_fn eval, void* context) {
    sv4_replace(dst, llg_dyn_reduce_with(array, operation, result_width, result_signed, result_two_state, eval, context));
}
void llg_queue_stream_to(sv4_t* dst, const llg_queue_t* queue, uint32_t slice, int right_to_left, int selector_kind, const sv4_t* first, const sv4_t* second) {
    sv4_replace(dst, llg_queue_stream(queue, slice, right_to_left, selector_kind, *first, *second));
}
void llg_queue_get_to(sv4_t* dst, const llg_queue_t* queue, const sv4_t* index) {
    sv4_replace(dst, llg_queue_get(queue, *index));
}
void llg_queue_pop_front_to(sv4_t* dst, llg_queue_t* queue) {
    sv4_replace(dst, llg_queue_pop_front(queue));
}
void llg_queue_pop_back_to(sv4_t* dst, llg_queue_t* queue) {
    sv4_replace(dst, llg_queue_pop_back(queue));
}
void llg_queue_front_to(sv4_t* dst, const llg_queue_t* queue) {
    sv4_replace(dst, llg_queue_front(queue));
}
void llg_queue_back_to(sv4_t* dst, const llg_queue_t* queue) {
    sv4_replace(dst, llg_queue_back(queue));
}
void llg_queue_reduce_to(sv4_t* dst, const llg_queue_t* queue, int operation) {
    sv4_replace(dst, llg_queue_reduce(queue, operation));
}
void llg_queue_reduce_with_to(sv4_t* dst, const llg_queue_t* queue, int operation, uint32_t result_width, int8_t result_signed, int result_two_state, llg_container_eval_fn eval, void* context) {
    sv4_replace(dst, llg_queue_reduce_with(queue, operation, result_width, result_signed, result_two_state, eval, context));
}
void llg_queue_cell_read_to(sv4_t* dst, const void* cell) {
    sv4_replace(dst, llg_queue_cell_read(cell));
}
void llg_queue_ref_read_to(sv4_t* dst, const llg_queue_t* queue, uint64_t identity) {
    sv4_replace(dst, llg_queue_ref_read(queue, identity));
}
void llg_assoc_value_get_integral_to(sv4_t* dst, const llg_assoc_value_t* array, const sv4_t* key) {
    sv4_replace(dst, llg_assoc_value_get_integral(array, *key));
}
void llg_assoc_value_get_nested_integral_to(sv4_t* dst, const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    sv4_replace(dst, llg_assoc_value_get_nested_integral(array, indices, count));
}
void llg_assoc_value_at_to(sv4_t* dst, const llg_assoc_t* array, size_t index) {
    sv4_replace(dst, llg_assoc_value_at(array, index));
}
void llg_assoc_reduce_to(sv4_t* dst, const llg_assoc_t* array, int operation) {
    sv4_replace(dst, llg_assoc_reduce(array, operation));
}
void llg_assoc_reduce_with_to(sv4_t* dst, const llg_assoc_t* array, int operation, uint32_t result_width, int8_t result_signed, int result_two_state, llg_container_eval_fn eval, void* context) {
    sv4_replace(dst, llg_assoc_reduce_with(array, operation, result_width, result_signed, result_two_state, eval, context));
}
void llg_assoc_get_integral_to(sv4_t* dst, const llg_assoc_t* array, const sv4_t* key) {
    sv4_replace(dst, llg_assoc_get_integral(array, *key));
}
void llg_assoc_get_string_to(sv4_t* dst, const llg_assoc_t* array, const void* key, size_t key_length) {
    sv4_replace(dst, llg_assoc_get_string(array, key, key_length));
}
void llg_dyn_value_get_string_to(llg_string_t* dst, const llg_dyn_value_array_t* array, const sv4_t* index) {
    llg_string_replace(dst, llg_dyn_value_get_string(array, *index));
}
void llg_dyn_value_get_nested_string_to(llg_string_t* dst, const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count) {
    llg_string_replace(dst, llg_dyn_value_get_nested_string(array, indices, count));
}
void llg_queue_value_get_string_to(llg_string_t* dst, const llg_queue_value_array_t* queue, const sv4_t* index) {
    llg_string_replace(dst, llg_queue_value_get_string(queue, *index));
}
void llg_queue_value_get_nested_string_to(llg_string_t* dst, const llg_queue_value_array_t* queue, const sv4_t* indices, size_t count) {
    llg_string_replace(dst, llg_queue_value_get_nested_string(queue, indices, count));
}
void llg_assoc_value_get_integral_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const sv4_t* key) {
    llg_string_replace(dst, llg_assoc_value_get_integral_string(array, *key));
}
void llg_assoc_value_get_nested_integral_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    llg_string_replace(dst, llg_assoc_value_get_nested_integral_string(array, indices, count));
}
void llg_assoc_value_get_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const void* key, size_t key_length) {
    llg_string_replace(dst, llg_assoc_value_get_string(array, key, key_length));
}
void llg_assoc_value_get_string_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const void* key, size_t key_length) {
    llg_string_replace(dst, llg_assoc_value_get_string_string(array, key, key_length));
}
