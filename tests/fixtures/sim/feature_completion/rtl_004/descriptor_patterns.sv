// SV2009 7.6, 10.4.2, 10.9.1: over-packed-limit snapshot construction.
module tb;
  typedef logic [16:0] lane_t;
  lane_t values[-1:65535];
  bit [16:0] binary_values[-1:65535];
  lane_t matrix[1:0][-1:32767];
  lane_t alternating[-65536:-1];
  initial begin
    values = '{default:17'h01234, -1:17'h1abcd, 65535:17'h1fedc};
    if (values[-1] !== 17'h1abcd || values[3] !== 17'h01234 || values[65535] !== 17'h1fedc) $fatal(1,"sparse keys");
    values = '{lane_t:values[-1], -1:values[65535]};
    if (values[-1] !== 17'h1fedc || values[42] !== 17'h1abcd) $fatal(1,"overlap snapshot");
    values <= '{default:values[-1], 0:values[42]};
    values[-1] = 17'h00001;
    values[42] = 17'h00002;
    #1;
    if (values[-1] !== 17'h1fedc || values[0] !== 17'h1abcd || values[65535] !== 17'h1fedc) $fatal(1,"NBA snapshot");
    binary_values = '{default:17'bxz000000000000001, 0:17'h1abcd};
    if (binary_values[-1] !== 17'h1 || binary_values[0] !== 17'h1abcd || binary_values[65535] !== 17'h1) $fatal(1,"state conversion");
    matrix = '{default:'{default:'1}};
    if (matrix[1][-1] !== 17'h1ffff || matrix[0][32767] !== 17'h1ffff) $fatal(1,"recursive default");
    values = '{65537{17'h1f00d}};
    if (values[-1] !== 17'h1f00d || values[65535] !== 17'h1f00d) $fatal(1,"large replication");
    alternating = '{32768{17'h00111,17'h10222}};
    if(alternating[-65536]!==17'h00111 || alternating[-65535]!==17'h10222 || alternating[-2]!==17'h00111 || alternating[-1]!==17'h10222) $fatal(1,"alternating replication");
    $display("descriptor_patterns=pass");
    $finish(0);
  end
endmodule
