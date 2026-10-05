// IEEE 1800-2009 11.4.14.4, 13.4: legal, but a function call inside an
// expression has no statement after it to run the checked unpack of a
// runtime `with` copy-out target; it keeps an explicit diagnostic. The same
// call as a statement is supported (`copyout_with`).
module tb;
  logic [7:0] arr [0:7];
  int n, r;
  function automatic int f(output logic [15:0] v);
    v = 16'h1234;
    return 1;
  endfunction
  initial begin
    n = 1;
    r = f({>>{arr with [n +: 2]}}) + 1;
    $finish(0);
  end
endmodule
