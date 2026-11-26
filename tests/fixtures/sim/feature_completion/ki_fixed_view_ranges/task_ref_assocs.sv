`timescale 1ns/1ns
class C;
  int a[2][int];
endclass
module tb;
  int m[2][int];
  string sm[3][string];
  C c;
  function automatic string show(const ref int t[int]);
    string text = "";
    foreach (t[k]) text = {text, $sformatf(" %0d:%0d", k, t[k])};
    return {"{", text, " }"};
  endfunction
  task automatic fill(ref int t[int]);
    t[5] = 1;
    #1 t[6] = 2;
    $display("%0t in task num=%0d", $time, m[1].num());
    #1 t.delete(5);
    #1;
  endtask
  task automatic names(ref string t[string], input string k);
    #1 t[k] = {k, "!"};
    #1 t[{k, k}] = k;
  endtask
  task automatic add(ref int t[int], input int v);
    #v t[v] = v;
  endtask
  task automatic local_array();
    int la[2][int];
    fork
      begin
        wait (la[0].num() == 1);
        $display("%0t local woke %s", $time, show(la[0]));
      end
    join_none
    fill(la[0]);
    $display("%0t local after %s", $time, show(la[0]));
  endtask
  initial begin
    c = new;
    fork
      begin
        wait (m[1].num() == 2);
        $display("%0t woke num=2 %s", $time, show(m[1]));
        wait (m[1].num() == 1);
        $display("%0t woke num=1 %s", $time, show(m[1]));
      end
      begin
        wait (sm[2].num() == 2);
        $display("%0t sm num=2 %p", $time, sm[2]);
      end
      begin
        wait (c.a[1].num() == 1);
        $display("%0t c.a woke %s", $time, show(c.a[1]));
      end
    join_none
    #0;
    fork
      begin fill(m[1]); $display("%0t fill returned", $time); end
      names(sm[2], "k");
      add(c.a[1], 2);
    join
    $display("%0t after %s %s %p %s", $time, show(m[0]), show(m[1]), sm[2], show(c.a[1]));
    fork
      begin
        wait (m[0].num() == 1);
        $display("%0t first of two %s", $time, show(m[0]));
        wait (m[0].num() == 2);
        $display("%0t both %s", $time, show(m[0]));
      end
    join_none
    #0;
    fork
      add(m[0], 1);
      add(m[0], 3);
    join
    local_array();
    #1 $finish;
  end
endmodule
