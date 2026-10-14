// A single nonzero weight makes the choice deterministic: every execution
// takes that branch and no other body runs.
module tb;
  localparam int N = 64;
  int hits[3];
  int unchosen;
  int single_runs;
  int unsigned big = 32'hFFFF_FFFF;
  shortint signed s_two = 2;

  task automatic pick(input string label, input int unsigned w0, input int unsigned w1,
                      input int unsigned w2);
    hits = '{0, 0, 0};
    repeat (N) begin
      randcase
        w0 : hits[0]++;
        w1 : hits[1]++;
        w2 : hits[2]++;
      endcase
    end
    $display("%s hits=%0d,%0d,%0d", label, hits[0], hits[1], hits[2]);
  endtask

  initial begin
    pick("first ", 1, 0, 0);
    pick("middle", 0, 7, 0);
    pick("last  ", 0, 0, big);
    hits = '{0, 0, 0};
    repeat (N) begin
      randcase
        0 : hits[0]++;
        5 : hits[1]++;
        0 : hits[2]++;
      endcase
    end
    $display("const  hits=%0d,%0d,%0d", hits[0], hits[1], hits[2]);
    repeat (N) begin
      randcase
        1 : single_runs++;
      endcase
    end
    $display("one item runs=%0d", single_runs);
    // A null body is a legal branch; the chosen weight comes from a signed
    // short integer.
    repeat (N) begin
      randcase
        0 : unchosen++;
        s_two : ;
        0 : unchosen++;
      endcase
    end
    $display("unchosen bodies=%0d", unchosen);
    $finish;
  end
endmodule
