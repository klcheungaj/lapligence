// SIM-015: forked children store process::self() into fixed, dynamic, queue
// and associative (integral and string key) arrays; the handles pass through
// task formals and are suspended, resumed, awaited and killed independently
// (SV 9.7, 7.4-7.10, 13.5).
module tb;
  process fixed_h[3];
  process dyn_h[];
  process queue_h[$];
  process assoc_h[int];
  process named_h[string];
  int done[5];

  task automatic show(input string label, input process p);
    $display("%s %s", label, p.status().name());
  endtask

  task automatic control(input process p, input int op);
    case (op)
      0: p.suspend();
      1: p.resume();
      2: p.kill();
      default: p.await();
    endcase
  endtask

  task automatic pick(input int k, output process p);
    case (k)
      0: p = fixed_h[2];
      1: p = dyn_h[0];
      2: p = queue_h[0];
      3: p = assoc_h[30];
      default: p = named_h["last"];
    endcase
  endtask

  initial begin
    process h;
    dyn_h = new[1];
    for (int i = 0; i < 5; i++)
      fork
        automatic int k = i;
        begin
          case (k)
            0: fixed_h[2] = process::self();
            1: dyn_h[0] = process::self();
            2: queue_h.push_back(process::self());
            3: assoc_h[30] = process::self();
            default: named_h["last"] = process::self();
          endcase
          #(10 * (k + 1));
          done[k] = 1;
        end
      join_none
    #1;
    for (int k = 0; k < 5; k++) begin
      pick(k, h);
      show($sformatf("t1 k%0d", k), h);
    end
    control(fixed_h[2], 0);
    control(dyn_h[0], 0);
    control(queue_h[0], 2);
    show("fixed", fixed_h[2]);
    show("dyn", dyn_h[0]);
    show("queue", queue_h[0]);
    #14;
    show("t15 fixed", fixed_h[2]);
    control(fixed_h[2], 1);
    control(fixed_h[2], 3);
    $display("fixed done=%0d at %0d", done[0], $time);
    control(dyn_h[0], 1);
    show("t15 dyn", dyn_h[0]);
    control(assoc_h[30], 3);
    $display("assoc done=%0d at %0d", done[3], $time);
    control(named_h["last"], 0);
    control(named_h["last"], 2);
    for (int k = 0; k < 5; k++) begin
      pick(k, h);
      show($sformatf("end k%0d", k), h);
    end
    $display("done %0d %0d %0d %0d %0d", done[0], done[1], done[2], done[3], done[4]);
    $finish;
  end
endmodule
