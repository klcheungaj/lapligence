// A production that rand join interleaves has one rule (decision S16-D5):
// alternatives are a located error.
module tb;
  initial randsequence (main)
    main : rand join a b ;
    a : x | y ;
    b : x ;
    x : { $display("x"); } ;
    y : { $display("y"); } ;
  endsequence
endmodule
