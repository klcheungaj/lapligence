`ifdef LLG_ADV032_NEVER_DEFINED
`default_decay_time 100
`delay_mode_path
`endif
module tb;
  initial begin
    $display("inactive directives are not reported");
    $finish;
  end
endmodule
