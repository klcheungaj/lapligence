// SV2009 sections 6.7, 7.4: fixed arrays of recursive record nets.
module tb;
  typedef struct { logic [7:0] word; logic [7:0] bytes [3:2]; logic signed [3:0] tag; } record_t;
  wire record_t values [-1:0];
  logic [7:0] source;
  assign values[-1].word = source;
  assign values[-1].tag = 4'he;
  assign values[0].word = 8'h42;
  assign values[0].tag = 4'h3;
  assign values[-1].bytes[3] = 8'h5a;
  assign values[-1].bytes[2] = source;
  assign values[0].bytes[3] = 8'h11;
  assign values[0].bytes[2] = 8'h22;
  initial begin
    source = 8'ha5;
    #1; $display("array=%h:%0d %h:%0d", values[-1].word, values[-1].tag, values[0].word, values[0].tag);
    $display("nested=%h:%h %h:%h", values[-1].bytes[3], values[-1].bytes[2], values[0].bytes[3], values[0].bytes[2]);
    source[3:0] = 4'hc;
    #1; $display("changed=%h nested=%h", values[-1].word, values[-1].bytes[2]);
    $finish(0);
  end
endmodule
