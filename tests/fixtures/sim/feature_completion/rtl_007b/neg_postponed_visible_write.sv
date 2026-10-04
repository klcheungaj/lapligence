// Negative (SV 4.4.2.9): a $strobe argument evaluated in the Postponed region
// cannot write a variable its helper does not own.
module tb;
  int a = 1, calls = 0;
  function int f(input int x);
    calls++;
    return x;
  endfunction
  initial begin
    $strobe("%0d", f(a));
    #1 $finish(0);
  end
endmodule
