// Decision S13-D5: a deferred statement that executes under a static
// initializer is a run-time error. Here the initializer calls a context DPI-C
// import whose foreign code calls an export holding a fork-join_none; the
// foreign code may call any export, so every export is checked.
//
// IEEE 1800-2009 13.4.4 (SystemVerilog-1800-2009.txt L18993-18997) lets a
// function schedule such work only when its calling thread was created by an
// initial or always procedure or a fork of one. 35.5.3 (L55775-55777): "the
// called import subroutine could make an inter-language call to an export DPI
// subroutine".
module tb;
  import "DPI-C" context function int s13_relay(int n);
  export "DPI-C" function s13_spawn;
  int hits;
  function int s13_spawn(int n);
    fork
      hits = n;
    join_none
    return n + 1;
  endfunction
  int started = s13_relay(4);
  initial #1 $display("%0d %0d", started, hits);
endmodule
