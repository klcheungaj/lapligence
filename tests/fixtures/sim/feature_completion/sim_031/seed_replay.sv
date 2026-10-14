// A seed replays the same selections. The picks themselves depend on the
// random generator, so the expected output holds only the relations; the
// checksum line is implementation dependent and the suite requires it to be
// identical across optimizer modes and value backends.
module tb;
  localparam int N = 96;
  int picks[$];
  int saved_a[$];
  int saved_b[$];
  int saved_c[$];
  int saved_wa[$];
  int saved_wb[$];

  task automatic draw(input int seed);
    picks.delete();
    void'($urandom(seed));
    repeat (N) begin
      randcase
        3 : picks.push_back(0);
        1 : picks.push_back(1);
        4 : picks.push_back(2);
      endcase
    end
  endtask

  // The 100-bit weights take the arbitrary-width path, which must replay too.
  task automatic draw_wide(input int seed);
    logic [99:0] w = 100'd1 << 80;
    picks.delete();
    void'($urandom(seed));
    repeat (N) begin
      randcase
        w : picks.push_back(0);
        w : picks.push_back(1);
      endcase
    end
  endtask

  function automatic bit same(input int x[$], input int y[$]);
    if (x.size() != y.size()) return 0;
    foreach (x[i]) if (x[i] != y[i]) return 0;
    return 1;
  endfunction

  function automatic int checksum(input int x[$]);
    int sum = 0;
    foreach (x[i]) sum = sum * 3 + x[i];
    return sum;
  endfunction

  initial begin
    draw(42);
    saved_a = picks;
    draw(42);
    saved_b = picks;
    draw(43);
    saved_c = picks;
    $display("replay equal: %s", same(saved_a, saved_b) ? "yes" : "no");
    $display("other seed differs: %s", same(saved_a, saved_c) ? "no" : "yes");
    draw_wide(7);
    saved_wa = picks;
    draw_wide(7);
    saved_wb = picks;
    $display("wide replay equal: %s", same(saved_wa, saved_wb) ? "yes" : "no");
    $display("checksum %0d %0d", checksum(saved_a), checksum(saved_wa));
    $finish;
  end
endmodule
