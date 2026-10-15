// A return in a value-returning production needs an expression (IEEE
// 1800-2009 18.17.7, "just like non-void functions").
module tb;
  initial randsequence (main)
    main : p ;
    int p : { return; } ;
  endsequence
endmodule
