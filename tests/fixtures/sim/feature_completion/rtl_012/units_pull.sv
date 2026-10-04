// RTL-012: this unit leaves `unconnected_drive pull1` active at its end.
`unconnected_drive pull1
module ua(input a);
  initial #1 $display("ua %v", a);
endmodule
