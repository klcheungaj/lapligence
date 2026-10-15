// llg interleaves only non-recursive productions.
module tb;
  int n;
  initial randsequence (main)
    main : rand join a b ;
    a : { n++; } a := 1 | { } := 1 ;
    b : { $display("b"); } ;
  endsequence
endmodule
