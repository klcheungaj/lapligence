// Allocation failure matrix: file descriptors, formatted writes and reads,
// memory images, string scanning and plusargs.
`timescale 1ns / 1ns
module tb;
  integer file, channel, code;
  logic [15:0] words[8];
  logic [99:0] wide_words[4];
  string line, name, text;
  int number, count;
  real ratio;
  logic [31:0] value;

  initial begin
    file = $fopen("alloc_files.txt", "w");
    channel = $fopen("alloc_files.log");
    for (int i = 0; i < 8; i++) $fdisplay(file, "%h // word %0d", 16'(i * 4369), i);
    $fwrite(file, "@2 %h\n", 16'hbeef);
    $fdisplay(channel | 32'd1, "to both %0d", 7);
    $fstrobe(channel, "strobe %0t", $time);
    $fflush(file);
    #1;
    $fclose(file);
    $fclose(channel);
    $readmemh("alloc_files.txt", words, 0, 7);
    $display("words %h %h %h", words[1], words[2], words[7]);
    file = $fopen("alloc_files.txt", "r");
    count = 0;
    while ($fgets(line, file) > 0) count += line.len();
    $display("lines %0d %0d", count, $feof(file));
    code = $rewind(file);
    code = $fscanf(file, "%h // %s %d", value, name, number);
    $display("scan %0d %h %s %0d %0d", code, value, name, number, $ftell(file));
    code = $fseek(file, 0, 0);
    code = $fgetc(file);
    code = $ungetc(code, file);
    code = $fread(wide_words, file);
    $display("read %0d %0d", code, $fgetc(file) != -1);
    $fclose(file);
    file = $fopen("alloc_files.bin", "wb");
    $fwrite(file, "%u%z", 32'h61626364, 32'h31323334);
    $fclose(file);
    $writememh("alloc_files.mem", words);
    $readmemh("alloc_files.mem", words);
    $writememb("alloc_files.mem", wide_words, 1, 2);
    $sformat(text, "%0d %s %h", 42, "answer", 12'habc);
    code = $sscanf(text, "%d %s %h", number, name, value);
    $display("sscanf %0d %0d %s %h", code, number, name, value);
    if ($value$plusargs("ratio=%f", ratio)) $display("ratio %0.2f", ratio);
    if ($value$plusargs("name=%s", name)) $display("name %s", name);
    if ($value$plusargs("wide=%h", wide_words[0])) $display("wide %h", wide_words[0]);
    $display("plus %0d %0d", $test$plusargs("flag"), $test$plusargs("absent"));
    file = $fopen("alloc_files.missing", "r");
    $display("missing %0d", file);
    $display("done");
    $finish;
  end
endmodule
