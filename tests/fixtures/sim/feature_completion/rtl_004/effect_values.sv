// SV2009 10.9.1 leaves type/default/replication multiplicity undefined.
// Each effect is idempotent and each return independent of invocation count.
module tb;
  typedef logic [7:0] lane_t;
  typedef struct packed { lane_t a; lane_t b; } packed_t;
  lane_t a[3], b[4];
  packed_t record_value;
  int observed;
  function automatic lane_t effect();
    observed = 1;
    return 8'h35;
  endfunction
  initial begin
    observed = 0;
    a = '{default:effect()};
    if (a[0] !== 8'h35 || a[1] !== 8'h35 || a[2] !== 8'h35 || observed != 1) $fatal(1,"default values");
    observed = 0;
    b = '{2{effect(),8'h7a}};
    if (b[0] !== 8'h35 || b[1] !== 8'h7a || b[2] !== 8'h35 || b[3] !== 8'h7a || observed != 1) $fatal(1,"replicated values");
    observed = 0;
    record_value = '{lane_t:effect()};
    if (record_value.a !== 8'h35 || record_value.b !== 8'h35 || observed != 1) $fatal(1,"type-key values");
    $display("effect_values=pass");
    $finish(0);
  end
endmodule
