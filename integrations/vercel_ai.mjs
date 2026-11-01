// A Vercel AI SDK agent that writes and runs Brevi through `brevi mcp`.
// npm i ai @ai-sdk/mcp @ai-sdk/anthropic; needs brevi on PATH and ANTHROPIC_API_KEY.
// usage: node vercel_ai.mjs "Read numbers from stdin and print their median"
import { generateText, stepCountIs } from 'ai';
import { anthropic } from '@ai-sdk/anthropic';
import { createMCPClient } from '@ai-sdk/mcp';
import { Experimental_StdioMCPTransport } from '@ai-sdk/mcp/mcp-stdio';

const client = await createMCPClient({
  transport: new Experimental_StdioMCPTransport({ command: 'brevi', args: ['mcp'] }),
});
try {
  const { text } = await generateText({
    model: anthropic('claude-sonnet-5-5'),
    tools: await client.tools(), // guide, check, run, test, patch
    stopWhen: stepCountIs(12),
    system: 'Write programs in Brevi. Read the guide tool once, then use run or check until the program works. Fix only the lines an error names.',
    prompt: process.argv.slice(2).join(' ') || 'Print the first 10 primes',
  });
  console.log(text);
} finally {
  await client.close();
}
