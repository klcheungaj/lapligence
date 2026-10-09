extern int tf_getp(int index);

int pli_user_calltf(void) { return tf_getp(1); }
