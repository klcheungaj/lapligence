// The returned expression must have the production type (IEEE 1800-2009
// 18.17.7).
module tb;
  class C;
  endclass
  initial randsequence (main)
    main : a ;
    int a : { C c = new; return c; } ;
  endsequence
endmodule
