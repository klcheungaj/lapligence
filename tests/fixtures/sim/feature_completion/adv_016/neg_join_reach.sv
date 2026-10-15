// A joined production must not generate another production of the same
// rand join in llg.
module tb;
  initial randsequence (main)
    main : rand join a b ;
    a : b ;
    b : { $display("b"); } ;
  endsequence
endmodule
