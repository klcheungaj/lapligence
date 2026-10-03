// SV2009 13.4.1, 6.24.1, 11.3.5: discarding or ignoring a function result
// never removes the call's observable effects (in either optimizer mode);
// only a short-circuited operand is not evaluated.
module tb;
  typedef logic [7:0] arr_t [0:1];
  typedef struct { logic [7:0] a; logic [7:0] b; } rec_t;
  int cnt = 0;
  arr_t sink_arr;
  logic [7:0] unused_value;
  function automatic int bump(input int k);
    cnt += k;
    return cnt;
  endfunction
  function automatic arr_t abump();
    cnt += 100;
    return '{8'd1, 8'd2};
  endfunction
  function automatic rec_t rbump();
    cnt += 1000;
    return '{8'd3, 8'd4};
  endfunction
  function automatic void vbump();
    cnt += 10000;
  endfunction
  function int static_bump();
    cnt += 100000;
    return 0;
  endfunction
  initial begin
    void'(bump(1));
    void'(abump());
    void'(rbump());
    vbump();
    void'(static_bump());
    unused_value = 8'(bump(2));
    sink_arr = abump();
    $display("discarded %0d", cnt);
    if (0 && bump(1000000) > 0) $display("never");
    if (1 || bump(1000000) > 0) $display("short_circuit %0d", cnt);
    $display("conditional %0d", (cnt > 0) ? cnt : bump(1000000));
    $finish(0);
  end
endmodule
