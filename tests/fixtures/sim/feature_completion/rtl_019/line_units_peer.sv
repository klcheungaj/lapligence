// Peer file for line_units.sv, admitted before it.
module peer;
`line 500 "peer_mapped.sv" 0
  task show;
    $display("%s:%0d", `__FILE__, `__LINE__);
  endtask
endmodule
