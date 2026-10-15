// llg supports only input production arguments (IEEE 1800-2009 18.17.7
// describes passing data down to a production).
module tb;
  int r;
  initial randsequence (main)
    main : p(r) ;
    p (output int o) : { o = 1; } ;
  endsequence
endmodule
