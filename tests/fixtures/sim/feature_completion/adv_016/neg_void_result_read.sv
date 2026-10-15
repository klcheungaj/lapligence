// Only productions that return a value declare an implicit variable
// (IEEE 1800-2009 18.17.7).
module tb;
  initial randsequence (main)
    main : a { $display("%0d", a); } ;
    a : { } ;
  endsequence
endmodule
