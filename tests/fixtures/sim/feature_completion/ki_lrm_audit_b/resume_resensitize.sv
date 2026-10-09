// resume() resensitizes a process suspended on an event expression; wait
// conditions and delays complete at resume (IEEE 1800-2009 9.7). Each waiter
// keeps its own log, so same-time wakes cannot reorder the output.
module tb;
  class C;
    int x;
  endclass

  C h;
  event e;
  logic clk;
  int v;
  process pe, pc, ph, pm, pw, pd;
  string le, lc, lh, lm, lw, ld;

  initial begin
    h = new;
    clk = 0;
    v = 0;
    fork
      begin
        pe = process::self();
        @e;
        le = $sformatf("%s %0d", le, $time);
      end
      begin
        pc = process::self();
        @(posedge clk);
        lc = $sformatf("%s %0d", lc, $time);
      end
      begin
        ph = process::self();
        @(h.x);
        lh = $sformatf("%s %0d", lh, $time);
      end
      begin
        pm = process::self();
        @(v or e);
        lm = $sformatf("%s %0d", lm, $time);
      end
      begin
        pw = process::self();
        wait (v == 1);
        lw = $sformatf("%s %0d", lw, $time);
      end
      begin
        pd = process::self();
        #3;
        ld = $sformatf("%s %0d", ld, $time);
      end
    join_none
    #1 begin
      pe.suspend();
      pc.suspend();
      ph.suspend();
      pm.suspend();
      pw.suspend();
      pd.suspend();
    end
    #1 begin
      ->e;
      clk = 1;
      h.x = 1;
      v = 1;
    end
    #1 $display("t3 %s %s %s %s %s %s", pe.status().name(), pc.status().name(),
                ph.status().name(), pm.status().name(), pw.status().name(),
                pd.status().name());
    #1 begin
      pe.resume();
      pc.resume();
      ph.resume();
      pm.resume();
      pw.resume();
      pd.resume();
    end
    #1 begin
      $display("t5 %s %s %s %s %s %s", pe.status().name(), pc.status().name(),
               ph.status().name(), pm.status().name(), pw.status().name(),
               pd.status().name());
      ->e;
    end
    #1 clk = 0;
    #1 clk = 1;
    #1 h.x = 1;
    #1 h.x = 0;
    #1 begin
      $display("event:%s", le);
      $display("posedge:%s", lc);
      $display("property:%s", lh);
      $display("or_list:%s", lm);
      $display("condition:%s", lw);
      $display("delay:%s", ld);
      $display("t10 %s %s %s %s %s %s", pe.status().name(), pc.status().name(),
               ph.status().name(), pm.status().name(), pw.status().name(),
               pd.status().name());
    end
    $finish(0);
  end
endmodule
