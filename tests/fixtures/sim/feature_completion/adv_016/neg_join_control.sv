// The rule of a production that rand join interleaves has no if, case or
// repeat item (decision S16-D5): a located error.
module tb;
  initial randsequence (main)
    main : rand join a b ;
    a : x repeat (2) y ;
    b : x ;
    x : { $display("x"); } ;
    y : { $display("y"); } ;
  endsequence
endmodule
