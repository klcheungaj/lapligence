// SV2009 13.5, 7.4: arrays beyond packed capacity (65,537 x 17 bits) cross
// nested ref forwarding, package const-ref, hierarchical module calls,
// named/default output arguments and early returns as descriptors.
package pkg;
  localparam int N = 65537;
  typedef logic [16:0] big_t [0:N-1];
  function automatic int psum3(const ref big_t a);
    return a[0] + a[N/2] + a[N-1];
  endfunction
endpackage

module child;
  import pkg::*;
  big_t store;
  function automatic void put(input big_t a);
    store = a;
  endfunction
  function automatic int peek(input int i);
    return store[i];
  endfunction
endmodule

module tb;
  import pkg::*;
  big_t a, b;
  child c0();
  function automatic void inner(ref big_t x, input int i, input logic [16:0] v);
    x[i] = v;
  endfunction
  function automatic void outer(ref big_t x);
    inner(x, 0, 17'd11);
    inner(x, N - 1, 17'd13);
  endfunction
  function automatic void outf(output big_t o, input logic [16:0] v = 17'd5);
    o[N/2] = v;
  endfunction
  function automatic big_t ident(input big_t x);
    return x;
  endfunction
  function automatic int first3(input big_t x);
    for (int i = 0; i < 8; i++) if (x[i] == 17'd3) return i;
    return -1;
  endfunction
  initial begin
    a[N/2] = 17'd7;
    outer(a);
    $display("nested_ref %0d", psum3(a));
    outf(b);
    $display("default_output %0d %h", b[N/2], b[0]);
    outf(.v(17'd9), .o(b));
    $display("named_output %0d", b[N/2]);
    b = ident(a);
    $display("identity %0d", pkg::psum3(b));
    c0.put(b);
    $display("hierarchical %0d %0d", c0.peek(0), c0.peek(N - 1));
    a[2] = 17'd3;
    $display("early_return %0d", first3(a));
    a[2] = 17'd4;
    $display("no_match %0d", first3(a));
    $finish(0);
  end
endmodule
