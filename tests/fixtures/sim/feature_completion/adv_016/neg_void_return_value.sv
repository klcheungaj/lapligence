// A production without a return type is void (IEEE 1800-2009 18.17.7).
module tb;
  initial randsequence (main)
    main : p ;
    p : { return 5; } ;
  endsequence
endmodule
