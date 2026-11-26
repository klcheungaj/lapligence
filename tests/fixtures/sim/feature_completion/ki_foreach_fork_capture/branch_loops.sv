// foreach loops inside fork branches, with no fork in the loop body that
// names the loop variable: each variable belongs to its branch's process.
`timescale 1ns/1ns
module tb;
  int fixed_a[3];
  int m2[2][2];
  int q[$];
  int aa[string];
  string s;
  task automatic fill();
    fork
      begin
        int loc[2];
        #1;
        foreach (loc[i]) loc[i] = 7 + i;
        $display("(T) loc %0d %0d", loc[0], loc[1]);
      end
    join_none
  endtask
  initial begin
    q = '{5, 6};
    aa["x"] = 1;
    aa["y"] = 2;
    s = "ab";
    fork
      begin
        #1;
        foreach (fixed_a[i]) fixed_a[i] = i * 10;
        foreach (m2[a, b]) m2[a][b] = a * 2 + b;
        foreach (q[j]) $display("(A) q[%0d]=%0d", j, q[j]);
      end
      begin
        #2;
        foreach (aa[k]) $display("(B) aa[%s]=%0d", k, aa[k]);
        foreach (s[n]) $display("(B) s[%0d]=%c", n, s[n]);
      end
      begin
        #3;
        foreach (fixed_a[m]) begin
          fork
            $display("(C) nested sees %0d", fixed_a[2]);
          join_none
        end
      end
    join_none
    fill();
    #5;
    $display("(D) fixed %0d %0d %0d m2 %0d %0d %0d %0d", fixed_a[0], fixed_a[1], fixed_a[2],
             m2[0][0], m2[0][1], m2[1][0], m2[1][1]);
    $finish;
  end
endmodule
