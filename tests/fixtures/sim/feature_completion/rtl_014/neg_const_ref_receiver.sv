// IEEE 1800-2009 13.5.2, 7.12.2: sorting mutates its receiver, so a const
// ref formal is a read-only receiver.
module tb;
  int a [0:19];
  function automatic void f(const ref int r [0:19]);
    r.sort();
  endfunction
  initial begin
    f(a);
    $finish;
  end
endmodule
