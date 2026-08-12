import type { ReactNode } from "react"

import { Badge } from "@astryxdesign/core/Badge"
import { Card } from "@astryxdesign/core/Card"
import { Heading } from "@astryxdesign/core/Heading"
import { HStack } from "@astryxdesign/core/HStack"
import { Layout, LayoutContent } from "@astryxdesign/core/Layout"
import { Section } from "@astryxdesign/core/Section"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import { Grid } from "../../ui/astryx/primitives/Grid"

export interface AstryxCliEvidence {
  readonly package: string
  readonly importPath: string
  readonly commands: readonly string[]
  readonly decisions: readonly string[]
}

export function CatalogPage({ title, description, children }: {
  readonly title: string
  readonly description: string
  readonly children: ReactNode
}) {
  return (
    <Layout height="auto">
      <LayoutContent padding={6} isScrollable={false} label={`${title} component catalog`}>
        <VStack gap={6} maxWidth="72rem">
          <VStack as="header" gap={2}>
            <Heading level={1}>{title}</Heading>
            <Text as="p" color="secondary" textWrap="pretty">{description}</Text>
            <HStack gap={1.5} wrap="wrap">
              <Badge variant="info" label="Astryx 0.3.0" />
              <Badge variant="neutral" label="CLI verified" />
              <Badge variant="neutral" label="Autodocs" />
            </HStack>
          </VStack>
          {children}
        </VStack>
      </LayoutContent>
    </Layout>
  )
}

export function CatalogSection({ title, description, children }: {
  readonly title: string
  readonly description?: string
  readonly children: ReactNode
}) {
  return (
    <Section variant="transparent" padding={0}>
      <VStack gap={3}>
        <VStack gap={1}>
          <Heading level={2}>{title}</Heading>
          {description ? <Text as="p" type="supporting">{description}</Text> : null}
        </VStack>
        {children}
      </VStack>
    </Section>
  )
}

export function CliEvidence({ evidence }: { readonly evidence: AstryxCliEvidence }) {
  return (
    <CatalogSection title="Astryx CLI evidence" description="这些命令是本页结构、组件选择和行为说明的来源；升级 Astryx 后重新运行。">
      <Grid label="Astryx CLI evidence" columns="auto-md" gap={3}>
        <Card variant="muted" padding={4}>
          <VStack gap={2}>
            <Text weight="semibold">Discovery commands</Text>
            {evidence.commands.map((command) => (
              <Text key={command} type="code" as="p" wordBreak="break-word">{command}</Text>
            ))}
          </VStack>
        </Card>
        <Card variant="muted" padding={4}>
          <VStack gap={2}>
            <Text weight="semibold">Adopted contract</Text>
            {evidence.decisions.map((decision) => (
              <Text key={decision} as="p" type="supporting">{decision}</Text>
            ))}
          </VStack>
        </Card>
      </Grid>
    </CatalogSection>
  )
}
