// SV 13.5.2: run-time-indexed member array elements of module, static and block records as `ref` actuals; the index is taken at the call.
module tb;
  class C; int v; function new(int x); v = x; endfunction endclass
  typedef struct { string s; int n; int a[3]; string t[2]; logic [7:0] b[1:3]; C h[2]; } rec_t;
  rec_t mr;
  int i = 1;
  int calls = 0;

  function automatic int next();
    calls++;
    return 1;
  endfunction

  task automatic ts(ref string x, input string v);
    x = v;
  endtask
  task automatic ti(ref int x, input int v);
    x = v;
  endtask
  task automatic tl(ref logic [7:0] x);
    x = 8'h5a;
  endtask
  function automatic int rd(const ref int x);
    return x;
  endfunction
  function automatic string rs(const ref string x);
    return x;
  endfunction
  task automatic th(ref C x, input int v);
    x = new(v);
  endtask
  task automatic late(ref int x, ref string s);
    #2;
    $display("late %0d %s", x, s);
    x = 99;
    s = "late";
  endtask
  task automatic wi(ref int x);
    @(x);
    $display("%0d wi %0d", $time, x);
  endtask
  task automatic ws(ref string x);
    wait (x == "go");
    $display("%0d ws %s", $time, x);
  endtask

  initial begin
    static rec_t st;
    rec_t bk;
    ts(mr.t[i], "one");
    i = 0;
    ts(mr.t[i], "zero");
    $display("%s %s", mr.t[0], mr.t[1]);
    ti(mr.a[i], 7);
    i = 2;
    ti(mr.a[i], 9);
    $display("%0d %0d %0d", mr.a[0], mr.a[1], mr.a[2]);
    i = 3;
    tl(mr.b[i]);
    $display("%h %h %h", mr.b[1], mr.b[2], mr.b[3]);
    i = 1;
    th(mr.h[i], 4);
    $display("%0d", mr.h[1].v);
    $display("%0d %s", rd(mr.a[i]), rs(mr.t[i]));

    ti(mr.a[next()], 12);
    $display("calls %0d a1 %0d", calls, mr.a[1]);

    i = 0;
    ts(st.t[i], "st");
    ti(st.a[i], 3);
    ts(bk.t[i], "bk");
    ti(bk.a[i], 4);
    $display("%s %0d %s %0d", st.t[0], st.a[0], bk.t[0], bk.a[0]);

    i = 1;
    fork
      late(mr.a[i], mr.t[i]);
      #1 begin
        i = 0;
        mr.a[1] = 50;
        mr.t[1] = "poke";
      end
    join
    $display("%0d %s %0d %s", mr.a[1], mr.t[1], mr.a[0], mr.t[0]);

    i = 2;
    fork
      wi(mr.a[i]);
      ws(mr.t[1]);
      begin
        #1 mr.a[1] = 1;
        #1 mr.t[0] = "go";
        #1 mr.a[2] = 2;
        #1 mr.t[1] = "go";
      end
    join
    i = 1;
    fork
      ws(mr.s);
      ws(mr.t[i]);
      begin
        #1 mr.s = "go";
        #1 mr.t[0] = "stop";
        #1 mr.t[1] = "stop";
        #1 mr.t[1] = "go";
      end
    join
    $finish;
  end
endmodule
