// A repeat count shall be integral (IEEE 1800-2009 18.17.4).
module tb;
  initial randsequence (main)
    main : repeat (2.5) a ;
    a : { } ;
  endsequence
endmodule
