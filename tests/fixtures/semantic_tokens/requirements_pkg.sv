// Package in its own project file for ST-28 and ST-29 (docs/semantic_tokens.md).
package req_ext_pkg;
  parameter int EXT_W = 8;
  localparam int EXT_L = 2;
  typedef logic [EXT_W-1:0] ext_t;
  typedef enum logic { EXT_OFF, EXT_ON } ext_e;
  function automatic int ext_inc(int v);
    return v + 1;
  endfunction
endpackage
