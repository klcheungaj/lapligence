// A task waits on a string or handle element through a `ref` formal: it wakes
// when that element changes, not on other elements (SV 9.4.2, 13.5.2).
module tb;
  class C;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass

  string fs[3];
  string qs[$];
  string as[string];
  C fc[2];
  C qc[$];
  int fired;

  task automatic w_str(ref string x);
    @(x);
    $display("%0d w_str %s", $time, x);
    wait (x == "go");
    $display("%0d w_str go", $time);
  endtask
  task automatic w_h(ref C x);
    @(x);
    $display("%0d w_h %0d", $time, x.v);
    wait (x.v == 3);
    $display("%0d w_h3", $time);
  endtask
  task automatic own_write(ref string x);
    #1 x = "self";
    fired = fired + 1;
    wait (x == "again");
    $display("%0d own %s", $time, x);
  endtask

  initial begin
    wait (fs[2] == "self");
    $display("%0d watcher %s", $time, fs[2]);
    wait (fs[2] == "parent");
    $display("%0d watcher %s", $time, fs[2]);
  end

  initial begin
    C c;
    qs = {"a", "b"};
    qc.push_back(null);
    as["k"] = "";
    fork
      w_str(fs[1]);
      w_str(qs[1]);
      w_str(as["k"]);
      w_h(fc[1]);
      w_h(qc[0]);
      own_write(fs[2]);
    join_none
    #1 fs[0] = "no";
    #1 fs[1] = "x";
    #1 fs[1] = "go";
    #1 qs[0] = "no";
    #1 qs[1] = "x";
    #1 qs.push_front("shift");
    #1 qs[2] = "go";
    #1 as["k"] = "x";
    #1 as["k"] = "go";
    #1 fc[0] = new(1);
    #1 fc[1] = new(1);
    #1 fc[1].v = 3;
    #1 c = new(1);
    qc[0] = c;
    #1 qc[0].v = 3;
    #1 fs[2] = "parent";
    #1 fs[2] = "again";
    #1 $display("done %0d", fired);
    $finish;
  end
endmodule
