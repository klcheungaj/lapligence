/* Destination-passing `_to` forms; private fragment, see value/destinations.h. */
void llg_fixed_array_stream_source_to(sv4_t* dst, const llg_fixed_array_t* array, int64_t declaration_left, int64_t declaration_right, uint32_t element_width, const sv4_t* fallback, int selector_kind, const sv4_t* first, const sv4_t* second) {
    sv4_replace(dst, llg_fixed_array_stream_source(array, declaration_left, declaration_right, element_width, *fallback, selector_kind, *first, *second));
}
void llg_fixed_array_compare_to(sv4_t* dst, const llg_fixed_array_t* p0, const llg_fixed_array_t* p1, int p2, int p3) {
    sv4_replace(dst, llg_fixed_array_compare(p0, p1, p2, p3));
}
void llg_net_alias_read_to(sv4_t* dst, llg_net_alias_t* alias) {
    sv4_replace(dst, llg_net_alias_read(alias));
}
void llg_q_full_to(sv4_t* dst, const sv4_t* q_id, sv4_t* status) {
    sv4_replace(dst, llg_q_full(*q_id, status));
}
void llg_urandom_to(sv4_t* dst) {
    sv4_replace(dst, llg_urandom());
}
void llg_urandom_seed_to(sv4_t* dst, const sv4_t* seed) {
    sv4_replace(dst, llg_urandom_seed(*seed));
}
void llg_urandom_range_to(sv4_t* dst, const sv4_t* max, const sv4_t* min, int has_min) {
    sv4_replace(dst, llg_urandom_range(*max, *min, has_min));
}
void llg_sequence_local_read_to(sv4_t* dst, void* attempt, uint32_t slot) {
    sv4_replace(dst, llg_sequence_local_read(attempt, slot));
}
void llg_system_to(sv4_t* dst, llg_string_t* command, int has_command) {
    sv4_replace(dst, llg_system(llg_string_take(command), has_command));
}
void llg_frame_read_value_to(sv4_t* dst, const llg_frame_t* frame, size_t slot) {
    sv4_replace(dst, llg_frame_read_value(frame, slot));
}
void llg_sampled_domain_past_to(sv4_t* dst, uint64_t identity, uint64_t ticks) {
    sv4_replace(dst, llg_sampled_domain_past(identity, ticks));
}
void llg_rt_ref_read_to(sv4_t* dst, const llg_ref_t* ref) {
    sv4_replace(dst, llg_rt_ref_read(ref));
}
void llg_process_get_randstate_to(llg_string_t* dst) {
    llg_string_replace(dst, llg_process_get_randstate());
}
void llg_process_handle_get_randstate_to(llg_string_t* dst, llg_process_handle_t* handle) {
    llg_string_replace(dst, llg_process_handle_get_randstate(handle));
}
void llg_string_format_typed_to(llg_string_t* dst, llg_string_t* format, llg_fmt_arg_t* args, int n, const char* scope) {
    llg_string_replace(dst, llg_string_format_typed(llg_string_take(format), args, n, scope));
}
