//! Key snapshots remain owned by the invoking frame across notifications.
pub(in crate::sim::emit_c) fn key_adapters() -> &'static str {
    "static sv4_t llg_owned_assoc_get_string(const llg_assoc_t *array, llg_string_t key) {\n\
     \x20   sv4_t result = llg_assoc_get_string(array, key.data, key.len);\n\
     \x20   return result;\n\
     }\n\
     static int llg_owned_assoc_exists_string(const llg_assoc_t *array, llg_string_t key) {\n\
     \x20   int result = llg_assoc_exists_string(array, key.data, key.len);\n\
     \x20   return result;\n\
     }\n\
     static int llg_owned_assoc_set_string(llg_assoc_t *array, llg_string_t key, sv4_t value) {\n\
     \x20   int result = llg_assoc_set_string(array, key.data, key.len, value);\n\
     \x20   return result;\n\
     }\n\
     static int llg_owned_assoc_delete_string(llg_assoc_t *array, llg_string_t key) {\n\
     \x20   int result = llg_assoc_delete_string(array, key.data, key.len);\n\
     \x20   return result;\n\
     }\n\
     static int llg_owned_assoc_traverse_string(const llg_assoc_t *array, llg_string_t *current, int direction) {\n\
     \x20   const unsigned char *bytes = NULL;\n\
     \x20   size_t length = 0;\n\
     \x20   int result;\n\
     \x20   switch (direction) {\n\
     \x20   case 0: result = llg_assoc_first_string(array, &bytes, &length); break;\n\
     \x20   case 1: result = llg_assoc_last_string(array, &bytes, &length); break;\n\
     \x20   case 2: result = llg_assoc_next_string(array, current->data, current->len, &bytes, &length); break;\n\
     \x20   default: result = llg_assoc_prev_string(array, current->data, current->len, &bytes, &length); break;\n\
     \x20   }\n\
     \x20   if (result) {\n\
     \x20       llg_string_t replacement = llg_string_bytes((const char *)bytes, length);\n\
     \x20       llg_string_move(current, replacement);\n\
     \x20   }\n\
     \x20   return result;\n\
     }\n\
     static int llg_owned_assoc_value_exists_string(const llg_assoc_value_t *array, llg_string_t key) {\n\
     \x20   int result = llg_assoc_value_exists_string(array, key.data, key.len);\n\
     \x20   return result;\n\
     }\n\
     static llg_string_t llg_owned_assoc_value_get_string(const llg_assoc_value_t *array, llg_string_t key) {\n\
     \x20   llg_string_t result = llg_assoc_value_get_string(array, key.data, key.len);\n\
     \x20   return result;\n\
     }\n\
     static double llg_owned_assoc_value_get_real(const llg_assoc_value_t *array, llg_string_t key) {\n\
     \x20   double result = llg_assoc_value_get_string_real(array, key.data, key.len);\n\
     \x20   return result;\n\
     }\n\
     static void *llg_owned_assoc_value_get_chandle(const llg_assoc_value_t *array, llg_string_t key) {\n\
     \x20   void *result = llg_assoc_value_get_string_chandle(array, key.data, key.len);\n\
     \x20   return result;\n\
     }\n\
     static int llg_owned_assoc_value_set_real(llg_assoc_value_t *array, llg_string_t key, double value) {\n\
     \x20   int result = llg_assoc_value_set_string_real(array, key.data, key.len, value);\n\
     \x20   return result;\n\
     }\n\
     static int llg_owned_assoc_value_set_string(llg_assoc_value_t *array, llg_string_t key, llg_string_t value) {\n\
     \x20   int result = llg_assoc_value_set_string_string(array, key.data, key.len, value);\n\
     \x20   return result;\n\
     }\n\
     static int llg_owned_assoc_value_set_chandle(llg_assoc_value_t *array, llg_string_t key, void *value) {\n\
     \x20   int result = llg_assoc_value_set_string_chandle(array, key.data, key.len, value);\n\
     \x20   return result;\n\
     }\n\
     static int llg_owned_assoc_value_delete_string(llg_assoc_value_t *array, llg_string_t key) {\n\
     \x20   int result = llg_assoc_value_delete_string(array, key.data, key.len);\n\
     \x20   return result;\n\
     }\n\
     static int llg_owned_assoc_value_traverse_string(const llg_assoc_value_t *array, llg_string_t *current, int direction) {\n\
     \x20   const unsigned char *bytes = NULL;\n\
     \x20   size_t length = 0;\n\
     \x20   int result;\n\
     \x20   switch (direction) {\n\
     \x20   case 0: result = llg_assoc_value_first_string(array, &bytes, &length); break;\n\
     \x20   case 1: result = llg_assoc_value_last_string(array, &bytes, &length); break;\n\
     \x20   case 2: result = llg_assoc_value_next_string(array, current->data, current->len, &bytes, &length); break;\n\
     \x20   default: result = llg_assoc_value_prev_string(array, current->data, current->len, &bytes, &length); break;\n\
     \x20   }\n\
     \x20   if (result) {\n\
     \x20       llg_string_t replacement = llg_string_bytes((const char *)bytes, length);\n\
     \x20       llg_string_move(current, replacement);\n\
     \x20   }\n\
     \x20   return result;\n\
     }\n\n"
}
