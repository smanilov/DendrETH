import { promisify } from 'node:util';
import { exec as exec_ } from 'node:child_process';

import { compileNimFileToWasm } from '@dendreth/utils/ts-utils/compile-nim-to-wasm';
import { getCosmosContractArtifacts } from '@dendreth/utils/cosmos-utils/cosmos-utils';

const exec = promisify(exec_);

export async function compileVerifierNimFileToWasm(target: string) {
  const { contractDir } = await getCosmosContractArtifacts(target);
  const nimFilePath = contractDir + `/lib/nim/verify/verify.nim`;

  await compileNimFileToWasm(
    nimFilePath,
    `--nimcache:"${contractDir}"/nimcache --d:lightClientCosmos \
    -o:"${contractDir}/nimcache/nim_verifier.wasm"`,
  );
}

export async function compileVerifierParseDataTool(target: string) {
  const { rootDir, contractDir } = await getCosmosContractArtifacts(target);
  const compileParseDataTool = `nim c -d:nimOldCaseObjects -o:"${contractDir}/nimcache/" \
  "${rootDir}/tests/helpers/verifier-parse-data-tool/"${target}"/verifier_parse_data.nim" `;

  console.info(
    `Building 'verifier-parse-data' tool \n  ╰─➤ ${compileParseDataTool}`,
  );

  await exec(compileParseDataTool);
}

export async function compileVerifierContract(
  patch: string | null,
  target: string,
) {
  const { contractDir, wasmContractPath } = await getCosmosContractArtifacts(
    target,
  );

  let dockerPatch = '';
  if (patch !== null) {
    dockerPatch = `-v "${contractDir}-${patch}/Cargo.toml":/code/Cargo.toml \
                   -v "${contractDir}-${patch}/Cargo.lock":/code/Cargo.lock`;
  }

  const compileContractCommandVerify = `docker run -t --rm \
  -v "${contractDir}":/code \
  -v "${contractDir}"/../src:/code/src \
  ${dockerPatch} \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.12.11 .`;

  console.info(`Building the contract \n  ╰─➤ ${compileContractCommandVerify}`);

  await exec(compileContractCommandVerify);
  console.info(`Compiling the Verifier contract finished \n`);
  console.info(`The wasm contract file is at:`, wasmContractPath);
  return wasmContractPath;
}

export async function compileContractMain(
  patch: string | null,
  target: string,
) {
  await compileVerifierNimFileToWasm(target);
  await compileVerifierParseDataTool(target);
  await compileVerifierContract(patch, target);
}
