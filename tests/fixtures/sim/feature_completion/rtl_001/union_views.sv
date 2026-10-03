// SV2009 sections 7.3.1, 7.3.2: untagged union views share storage.
module tb;
  typedef enum logic signed [7:0] { NEG = -2 } signed_t;
  typedef union packed { logic [7:0] raw; signed_t signed_view; } packed_t;
  typedef union { logic [15:0] wide; logic [7:0] narrow; } unequal_t;
  packed_t equal_view, saved;
  unequal_t unequal_view;
  initial begin
    equal_view.signed_view = NEG;
    saved = equal_view;
    equal_view.raw[3:0] = 4'h1;
    unequal_view.wide = 16'habcd;
    $display("equal=%h signed=%0d saved=%h", equal_view.raw, equal_view.signed_view, saved.raw);
    $display("unequal=%h narrow=%h", unequal_view.wide, unequal_view.narrow);
    unequal_view.narrow = 8'h12;
    $display("selected=%h", unequal_view.wide);
    $finish(0);
  end
endmodule
