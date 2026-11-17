// SV 10.6.2, 6.4, 7.4.6: a forced element of a large static unpacked array,
// whose cells are allocated when first written, keeps its forced value
// through procedural writes, whole-array assignments and fills, a
// nonblocking whole-array assignment and an ordering method. The release
// leaves the value until the next write.
module tb;
  int big[100000];
  int src[100000];
  int i = 7;
  initial begin
    force big[i] = 70;
    big[7] = 1;
    #1 $display("s1 %0d %0d %0d", big[6], big[7], big[8]);
    src[7] = 1;
    src[8] = 2;
    big = src;
    #1 $display("s2 %0d %0d %0d", big[6], big[7], big[8]);
    big = '{default: 5};
    #1 $display("s3 %0d %0d %0d", big[6], big[7], big[8]);
    big.reverse();
    #1 $display("s4 %0d %0d %0d", big[7], big[99992], big[99993]);
    release big[i];
    #1 $display("s5 %0d", big[7]);
    big[7] = 3;
    #1 $display("s6 %0d", big[7]);
    i = 99999;
    force big[i] = 9;
    #1 $display("s7 %0d %0d", big[99998], big[99999]);
    #1 $display("s8 %0d %0d", big[99998], big[99999]);
    $display("done");
    $finish;
  end
  initial begin
    #7 big <= src;
  end
endmodule
