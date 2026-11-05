// S25-D8: a slice actual of `$strobe` is read at the end of the slot, and a
// `$monitor` over a slice reports each change of the slice value.
// 21.2.2: "The action shall occur just before simulation time is advanced and
// after all other events at that time have occurred so that the data written
// are sure to be the correct data for that simulation time."
// 21.2.3: "each time a variable or an expression in the argument list changes
// value [...] the entire argument list is displayed at the end of the time step".
// Some simulators copy a slice at the call and never read it again, while the
// same tools read a queue actual at the end of the slot; llg follows the clauses.
`timescale 1ns/1ns
module tb;
  int arr[4] = '{10, 20, 100, 200};
  int k = 1;
  function automatic int sumf(int x[2]);
    return x[0] + x[1];
  endfunction
  initial begin
    $strobe("%0d", sumf(arr[1:2]));
    arr[1] = 30;
    #1;
    $strobe("%0d", sumf(arr[k+:2]));
    k = 2;
    #1;
    arr[1] = 20;
    $monitor("m %0d", sumf(arr[1:2]));
    #1 arr[1] = 30;
    #1 arr[2] = 300;
    #1 arr[0] = 5;
    #1 $finish(0);
  end
endmodule
