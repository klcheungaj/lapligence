// RTL-018 library `rtl`: observes which macros reached this source. A
// macro-named include is admitted only when the macro is in scope.
`ifdef RTL018_LATE_HEADER
`include `RTL018_LATE_HEADER
`endif
module rtl018_unit_b(shared, map_only, cli, late);
  output [7:0] shared, map_only, cli, late;
`ifdef RTL018_SHARED
  assign shared = `RTL018_SHARED;
`else
  assign shared = 0;
`endif
`ifdef RTL018_MAP_ONLY
  assign map_only = 1;
`else
  assign map_only = 0;
`endif
`ifdef RTL018_CLI
  assign cli = `RTL018_CLI;
`else
  assign cli = 0;
`endif
`ifdef RTL018_LATE
  assign late = `RTL018_LATE;
`else
  assign late = 0;
`endif
endmodule
