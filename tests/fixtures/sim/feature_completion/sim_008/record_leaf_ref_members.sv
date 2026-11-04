// SV 13.5.2, 7.4.6: members of run-time-indexed record elements of module-like records, and indices selecting no element.
module tb;
  typedef struct { int n; string s; } in_t;
  typedef struct { in_t e[2]; string t[2]; logic [3:0] p[2]; } rec_t;
  rec_t mr;
  in_t ar[2];
  int i = 1;
  task automatic ts(ref string x, input string v);
    x = v;
  endtask
  task automatic ti(ref int x, input int v);
    x = v;
  endtask
  task automatic tl(ref logic [3:0] x);
    $display("%b", x);
    x = 4'b1010;
  endtask
  task automatic ws(ref string x);
    wait (x == "go");
    $display("%0d ws %s", $time, x);
  endtask
  initial begin
    static rec_t st;
    ts(mr.e[i].s, "es");
    ti(mr.e[i].n, 5);
    $display("[%s] [%0d] [%s] [%0d]", mr.e[0].s, mr.e[0].n, mr.e[1].s, mr.e[1].n);
    i = 0;
    ts(st.e[i].s, "st");
    ti(st.e[i].n, 6);
    $display("[%s] [%0d] [%s] [%0d]", st.e[0].s, st.e[0].n, st.e[1].s, st.e[1].n);
    // 7.4.6: an index selecting no element reads the default and writes nowhere.
    i = 5;
    ts(mr.t[i], "gone");
    tl(mr.p[i]);
    i = 'x;
    ti(mr.e[i].n, 8);
    $display("[%s] [%s] [%b] [%b] [%0d] [%0d]", mr.t[0], mr.t[1], mr.p[0], mr.p[1], mr.e[0].n, mr.e[1].n);
    i = 1;
    fork
      ws(ar[i].s);
      ws(mr.e[i].s);
      begin
        #1 ar[0].s = "go";
        #1 ar[1].s = "go";
        #1 mr.e[0].s = "go";
        #1 mr.e[1].s = "go";
      end
    join
    $finish;
  end
endmodule
