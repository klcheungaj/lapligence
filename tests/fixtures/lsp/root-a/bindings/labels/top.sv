// llg-lsp-fixture: root-a/bindings/labels/top.sv
module label_top;
  logic wa;

  label_child u_child(.clk(wa));
endmodule
