// SV2009 3.12.1: with separate compilation units, each file's
// compilation-unit scope is distinct; this file's `unit_k` and `unit_get` are
// not the ones declared in the file that instantiates `helper`.
int unit_k = 1;
function int unit_get();
  return unit_k * 10;
endfunction
module helper(output int o);
  int local_v = unit_get() + unit_k;
  assign o = local_v;
endmodule
