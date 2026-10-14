// Weights of every width: a single nonzero weight fixes the choice at 40, 64
// and 100 bits; equal wide weights both occur; the widest legal sum (two
// weights of 1048574 bits, a 1048575-bit exact sum) still selects.
module tb;
  int hits[2];
  logic [39:0] z40 = 0;
  logic [39:0] n40 = 40'hFF_FFFF_FFFF;
  logic [63:0] max64 = 64'hFFFF_FFFF_FFFF_FFFF;
  logic [62:0] max63 = 63'h7FFF_FFFF_FFFF_FFFF;
  logic [99:0] z100 = 0;
  logic [99:0] top100 = 100'd1 << 99;
  logic [1048573:0] huge_small = 1;
  logic [1048573:0] huge_zero = 0;

  task automatic show(input string label);
    $display("%s hits=%0d,%0d", label, hits[0], hits[1]);
    hits = '{0, 0};
  endtask

  initial begin
    repeat (50) begin
      randcase
        z40 : hits[0]++;
        n40 : hits[1]++;
      endcase
    end
    show("40-bit single    ");
    repeat (50) begin
      randcase
        max64 : hits[0]++;
        0 : hits[1]++;
      endcase
    end
    show("64-bit single    ");
    repeat (50) begin
      randcase
        z100 : hits[0]++;
        top100 : hits[1]++;
      endcase
    end
    show("100-bit single   ");
    repeat (400) begin
      randcase
        max63 : hits[0]++;
        max63 : hits[1]++;
      endcase
    end
    $display("63-bit pair both=%s total=%0d", (hits[0] > 0 && hits[1] > 0) ? "yes" : "no",
             hits[0] + hits[1]);
    hits = '{0, 0};
    repeat (400) begin
      randcase
        max64 : hits[0]++;
        max64 : hits[1]++;
      endcase
    end
    $display("64-bit pair both=%s total=%0d", (hits[0] > 0 && hits[1] > 0) ? "yes" : "no",
             hits[0] + hits[1]);
    hits = '{0, 0};
    repeat (400) begin
      randcase
        top100 : hits[0]++;
        top100 : hits[1]++;
      endcase
    end
    $display("100-bit pair both=%s total=%0d", (hits[0] > 0 && hits[1] > 0) ? "yes" : "no",
             hits[0] + hits[1]);
    hits = '{0, 0};
    repeat (3) begin
      randcase
        huge_zero : hits[0]++;
        huge_small : hits[1]++;
      endcase
    end
    show("1048574-bit      ");
    $finish;
  end
endmodule
