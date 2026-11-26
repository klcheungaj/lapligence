#include "sv_vpi_user.h"
#include "vpi_user.h"
#include <string.h>

static void show_one(const char *name) {
  s_vpi_error_info info;
  s_vpi_value value;
  char full[128];
  const char *text;
  vpiHandle handle = vpi_handle_by_name((PLI_BYTE8 *)name, NULL);
  (void)vpi_chk_error(&info);
  if (!handle) {
    vpi_printf("%s: none\n", name);
    return;
  }
  text = vpi_get_str(vpiFullName, handle);
  strncpy(full, text ? text : "(none)", sizeof full - 1);
  full[sizeof full - 1] = 0;
  (void)vpi_chk_error(&info);
  vpi_printf("%s: type %d name %s full %s", name, (int)vpi_get(vpiType, handle),
             vpi_get_str(vpiName, handle), full);
  value.format = vpiHexStrVal;
  value.value.str = NULL;
  vpi_get_value(handle, &value);
  if (!vpi_chk_error(&info) && value.value.str) {
    vpi_printf(" value %s", value.value.str);
  }
  vpi_printf(" automatic %d\n", (int)vpi_get(vpiAutomatic, handle));
}

static void list(const char *title, PLI_INT32 type, vpiHandle scope) {
  s_vpi_error_info info;
  vpiHandle item;
  vpiHandle items = scope ? vpi_iterate(type, scope) : NULL;
  (void)vpi_chk_error(&info);
  vpi_printf("%s:", title);
  while (items && (item = vpi_scan(items)) != NULL) {
    vpi_printf(" %s", vpi_get_str(vpiName, item));
  }
  vpi_printf("\n");
}

static PLI_INT32 show(PLI_BYTE8 *data) {
  static const char *const names[] = {
      "tb.p.pr",    "tb.p.pr.hi",  "tb.p.rec",     "tb.p.rec.a",  "tb.p.de.s",
      "tb.p.de.r.f", "tb.p.sa[1]", "tb.p.ra[1]",   "tb.p.ra[1].a", "tb.p.ss",
      "tb.C::pr",   "tb.C::pr.lo", "tb.C::rec.a",  "tb.C::de.r.a", "tb.C::ra[0].f",
      "tb.C::none", "tb.p.rec.none"};
  size_t index;
  s_vpi_error_info info;
  vpiHandle variable;
  vpiHandle object;
  (void)data;
  for (index = 0; index < sizeof names / sizeof names[0]; index++) {
    show_one(names[index]);
  }
  variable = vpi_handle_by_name((PLI_BYTE8 *)"tb.p", NULL);
  object = vpi_handle(vpiClassObj, variable);
  (void)vpi_chk_error(&info);
  list("object", vpiVariables, object);
  list("module", vpiVariables, vpi_handle_by_name((PLI_BYTE8 *)"tb", NULL));
  list("record", vpiMember, vpi_handle_by_name((PLI_BYTE8 *)"tb.p.de", NULL));
  return 0;
}

static void put(const char *name, PLI_INT32 integer) {
  s_vpi_error_info info;
  s_vpi_value value;
  vpiHandle handle = vpi_handle_by_name((PLI_BYTE8 *)name, NULL);
  value.format = vpiIntVal;
  value.value.integer = integer;
  vpi_put_value(handle, &value, NULL, vpiNoDelay);
  if (vpi_chk_error(&info)) {
    vpi_printf("put %s: error\n", name);
  }
}

static PLI_INT32 write_all(PLI_BYTE8 *data) {
  (void)data;
  put("tb.p.pr.hi", 9);
  put("tb.p.rec.a", 33);
  put("tb.C::rec.f", 0x66);
  put("tb.p.de.b", 77);
  put("tb.C::de.r.a", 88);
  put("tb.p.sa[1]", 55);
  put("tb.p.ra[1].a", 66);
  return 0;
}

static void install(void) {
  s_vpi_systf_data task;
  memset(&task, 0, sizeof task);
  task.type = vpiSysTask;
  task.tfname = (PLI_BYTE8 *)"$show";
  task.calltf = show;
  (void)vpi_register_systf(&task);
  task.tfname = (PLI_BYTE8 *)"$write_all";
  task.calltf = write_all;
  (void)vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {install, 0};
