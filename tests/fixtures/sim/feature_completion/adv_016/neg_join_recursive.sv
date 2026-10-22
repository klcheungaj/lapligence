// llg interleaves only non-recursive productions.
module tb;
  int n;
  initial randsequence (main)
    main : rand join a b ;
    a : { n++; } c ;
    b : { $display("b"); } ;
    c : a := 1 | { } := 1 ;
  endsequence
endmodule
