// SV 9.4.2: an event control on a string array element wakes on a change of the
// value of that element only. See the readme.
`timescale 1ns/1ns
module tb;
  string fs[4];
  string fi[4];
  string fo[2];
  string f2[2][2];
  string q[$];
  string d[];
  string aa[int];
  int ia[3];
  int i = 2;
  always @(fs[2]) $display("%0t fs2 %s", $time, fs[2]);
  always @(f2[1][0]) $display("%0t f2 %s", $time, f2[1][0]);
  always @(q[0]) $display("%0t q0 %s", $time, q[0]);
  always @(d[1]) $display("%0t d1 %s", $time, d[1]);
  always @(aa[3]) $display("%0t aa3 %s", $time, aa[3]);
  always @(fi[i]) $display("%0t fi %s", $time, fi[i]);
  always @(fo[0] or fo[1]) $display("%0t fo", $time);
  always @(ia[1]) $display("%0t ia1 %0d", $time, ia[1]);
  initial begin
    q.push_back("a"); d = new[2]; 
    #1 fs[2] = "x";
    #1 fs[1] = "other";
    #1 fs[2] = "x";
    #1 fs[2] = "y";
    #1 f2[1][0] = "z";
    #1 f2[0][0] = "other";
    #1 q[0] = "b";
    #1 q.push_front("c");
    #1 d[1] = "e";
    #1 d[0] = "other";
    #1 aa[3] = "g";
    #1 aa[4] = "other";
    #1 fi[2] = "m";
    #1 fi[1] = "n";
    #1 i = 1;
    #1 fi[2] = "other";
    #1 fi[1] = "w";
    #1 fo[0] = "a";
    #1 fo[1] = "b";
    #1 $finish;
  end
endmodule
