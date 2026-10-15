// rand join interleaves distinct productions in llg.
module tb;
  initial randsequence (main)
    main : rand join a a ;
    a : { $display("a"); } ;
  endsequence
endmodule
